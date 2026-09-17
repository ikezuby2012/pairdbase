use crate::features::connections::{
    domain::Connection,
    drivers::trait_::{
        DatabaseDriver, DatabaseMetadata, MongoCollectionInfo, MongoCollectionOptions,
        MongoCollectionStats, MongoCollectionType, MongoDatabaseInfo, MongoDatabaseStats,
        MongoFieldInfo, MongoIndexInfo, MongoIndexKey, MongoIndexOrder, MongoSchemaInfo,
        MongoTimeSeriesOptions, MongoValidator, MongoViewInfo
    },
};
use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use mongodb::bson::doc;
use mongodb::{
    bson::{Bson, Document},
    options::{ClientOptions, CreateCollectionOptions, ListCollectionsOptions},
    Client, Collection,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::versions::{MongoCapabilities, ServerVersion};

pub struct MongoDriver {
    client: Client,
    db_name: String,
    version: ServerVersion,
    capabilities: MongoCapabilities,
}

fn bson_type_name(value: &Bson) -> &'static str {
    match value {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Document(_) => "object",
        Bson::Array(_) => "array",
        Bson::Binary(_) => "binData",
        Bson::Undefined => "undefined",
        Bson::ObjectId(_) => "objectId",
        Bson::Boolean(_) => "bool",
        Bson::DateTime(_) => "date",
        Bson::Null => "null",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        Bson::Int32(_) => "int32",
        Bson::Timestamp(_) => "timestamp",
        Bson::Int64(_) => "int64",
        Bson::Decimal128(_) => "decimal128",
        Bson::Symbol(_) => "symbol",
        Bson::DbPointer(_) => "dbPointer",
        Bson::MinKey => "minKey",
        Bson::MaxKey => "maxKey",
        Bson::DateTime(_) => "date",
    }
}

#[derive(Debug, Default)]
struct FieldAccumulator {
    types: HashSet<String>,
    occurrences: u64,
    sample_value: Option<String>,
    nested: HashMap<String, FieldAccumulator>,
}

impl MongoDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let host = conn.host.as_deref().unwrap_or("localhost");
        let port = conn.port.unwrap_or(27017);
        let db_name = conn.database_name.clone().unwrap_or_else(|| "test".into());

        let url = match (&conn.username, &conn.password) {
            (Some(u), Some(p)) => format!("mongodb://{u}:{p}@{host}:{port}/{db_name}"),
            _ => format!("mongodb://{host}:{port}/{db_name}"),
        };

        let options = ClientOptions::parse(&url)
            .await
            .map_err(|e| e.to_string())?;

        let client = Client::with_options(options).map_err(|e| e.to_string())?;

        let server_info = Self::get_server_version(&client, &db_name).await?;

        let capabilities = MongoCapabilities::from_version(&server_info);

        Ok(Self {
            client,
            db_name,
            version: server_info,
            capabilities,
        })
    }

    async fn get_server_version(client: &Client, db_name: &str) -> Result<ServerVersion, String> {
        let result = client
            .database(db_name)
            .run_command(doc! {
                "buildInfo": 1
            })
            .await
            .map_err(|e| e.to_string())?;

        let raw = result
            .get_str("version")
            .map_err(|e| format!("MongoDB version not found: {e}"))?
            .to_string();

        let version = parse_server_version(&raw);

        Ok(version)
    }

    // ============================================================
    // DATABASE STATS
    // ============================================================

    async fn fetch_database_stats(&self) -> Result<MongoDatabaseStats, String> {
        let result = self
            .client
            .database(&self.db_name)
            .run_command(doc! {
                "dbStats": 1
            })
            .await
            .map_err(|e| e.to_string())?;

        Ok(MongoDatabaseStats {
            collections: result.get_i32("collections").ok().map(|v| v as i64),

            views: result.get_i32("views").ok().map(|v| v as i64),

            objects: result.get_i64("objects").ok(),

            data_size_bytes: result.get_i64("dataSize").ok(),

            storage_size_bytes: result.get_i64("storageSize").ok(),

            indexes: result.get_i32("indexes").ok().map(|v| v as i64),

            index_size_bytes: result.get_i64("indexSize").ok(),
        })
    }

    // ============================================================
    // COLLECTIONS
    // ============================================================

    async fn fetch_collections(&self) -> Result<Vec<MongoCollectionInfo>, String> {
        let db = self.client.database(&self.db_name);

        let mut cursor = db
            .list_collections()
            .with_options(ListCollectionsOptions::builder().build())
            .await
            .map_err(|e| e.to_string())?;

        let mut collections = Vec::new();

        while cursor.has_next() {
            let info = cursor
                .next()
                .await
                .ok_or_else(|| "Expected collection info".to_string())?
                .map_err(|e| e.to_string())?;

            let name = info.name;

            let collection_type = if info.options.view_on.is_some() {
                MongoCollectionType::View
            } else if info.options.capped.is_some() {
                MongoCollectionType::Capped
            } else if info.options.timeseries.is_some() {
                MongoCollectionType::TimeSeries
            } else {
                MongoCollectionType::Collection
            };

            // Views are handled separately.
            if matches!(collection_type, MongoCollectionType::View) {
                continue;
            }

            let options = self.parse_collection_options(&info.options);

            let stats = self.fetch_collection_stats(&name).await.ok();

            let fields = self.infer_document_schema(&name).await.unwrap_or_default();

            let indexes = self.fetch_indexes(&name).await.unwrap_or_default();

            let validator = self.fetch_validator(&name).await.ok().flatten();

            collections.push(MongoCollectionInfo {
                name: name.into(),
                collection_type,
                options,
                stats,
                fields,
                indexes,
                validator,
            });
        }

        Ok(collections)
    }

    // ============================================================
    // COLLECTION OPTIONS
    // ============================================================

    fn parse_collection_options_v2(&self, options: Option<&Document>) -> MongoCollectionOptions {
        let Some(options) = options else {
            return MongoCollectionOptions::default();
        };

        let capped = options.get_bool("capped").unwrap_or(false);

        let max_size_bytes = options.get_i64("size").ok();

        let max_documents = options.get_i64("max").ok();

        let timeseries = options
            .get_document("timeseries")
            .ok()
            .map(|ts| MongoTimeSeriesOptions {
                time_field: ts.get_str("timeField").unwrap_or_default().to_string(),

                meta_field: ts.get_str("metaField").ok().map(str::to_string),

                granularity: ts.get_str("granularity").ok().map(str::to_string),
            });

        MongoCollectionOptions {
            capped,
            max_size_bytes,
            max_documents,
            timeseries,
        }
    }

    fn parse_collection_options(
        &self,
        options: &CreateCollectionOptions,
    ) -> MongoCollectionOptions {
        MongoCollectionOptions {
            capped: options.capped.unwrap_or(false),
            max_size_bytes: options.size.map(|v| v as i64),
            max_documents: options.max.map(|f| f as i64),
            timeseries: options
                .timeseries
                .as_ref()
                .map(|ts| MongoTimeSeriesOptions {
                    time_field: ts.time_field.clone(),
                    meta_field: ts.meta_field.clone(),
                    granularity: ts.granularity.as_ref().map(|g| format!("{:?}", g)),
                }),
        }
    }

    // ============================================================
    // COLLECTION STATS
    // ============================================================

    async fn fetch_collection_stats(
        &self,
        collection_name: &str,
    ) -> Result<MongoCollectionStats, String> {
        let result = self
            .client
            .database(&self.db_name)
            .run_command(doc! {
                "collStats": collection_name
            })
            .await
            .map_err(|e| e.to_string())?;

        Ok(MongoCollectionStats {
            document_count: result.get_i64("count").ok(),

            size_bytes: result.get_i64("size").ok(),

            storage_size_bytes: result.get_i64("storageSize").ok(),

            total_index_size_bytes: result.get_i64("totalIndexSize").ok(),

            index_count: result
                .get_document("indexSizes")
                .ok()
                .map(|doc| doc.len() as i32),
        })
    }

    // ============================================================
    // DOCUMENT SCHEMA INFERENCE
    // ============================================================

    async fn infer_document_schema(
        &self,
        collection_name: &str,
    ) -> Result<Vec<MongoFieldInfo>, String> {
        let collection: Collection<Document> = self
            .client
            .database(&self.db_name)
            .collection(collection_name);

        let mut cursor = collection
            .aggregate(vec![doc! {
                "$sample": {
                    "size": 100
                }
            }])
            .await
            .map_err(|e| e.to_string())?;

        let mut fields: HashMap<String, FieldAccumulator> = HashMap::new();

        let mut sampled_documents = 0u64;

        use futures::StreamExt;

        while let Some(result) = cursor.next().await {
            let document = result.map_err(|e| e.to_string())?;

            sampled_documents += 1;

            self.collect_document_fields(&document, "", &mut fields);
        }

        Ok(fields
            .into_iter()
            .map(|(name, accumulator)| MongoFieldInfo {
                name,
                bson_types: accumulator.types.clone().into_iter().collect(),

                nullable: accumulator.types.contains("null"),

                optional: accumulator.occurrences < sampled_documents,

                occurrences: accumulator.occurrences,

                sample_value: accumulator.sample_value,

                nested: accumulator
                    .nested
                    .into_iter()
                    .map(|(name, accumulator)| MongoFieldInfo {
                        name,
                        bson_types: accumulator.types.clone().into_iter().collect(),

                        nullable: accumulator.types.contains("null"),

                        optional: accumulator.occurrences < sampled_documents,

                        occurrences: accumulator.occurrences,

                        sample_value: accumulator.sample_value,

                        nested: vec![],
                    })
                    .collect(),
            })
            .collect())
    }

    fn collect_document_fields(
        &self,
        document: &Document,
        prefix: &str,
        fields: &mut HashMap<String, FieldAccumulator>,
    ) {
        for (key, value) in document {
            let field_name = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            let type_name = bson_type_name(value);

            let accumulator = fields.entry(field_name.clone()).or_default();

            accumulator.types.insert(type_name.to_string());

            accumulator.occurrences += 1;

            if accumulator.sample_value.is_none() {
                accumulator.sample_value = Some(format!("{:?}", value));
            }

            // Recursively inspect nested documents.
            if let Bson::Document(nested) = value {
                self.collect_document_fields(nested, &field_name, fields);
            }

            // Inspect documents inside arrays.
            if let Bson::Array(items) = value {
                for item in items {
                    if let Bson::Document(nested) = item {
                        self.collect_document_fields(nested, &field_name, fields);
                    }
                }
            }
        }
    }

    // ============================================================
    // INDEXES
    // ============================================================

    async fn fetch_indexes(&self, collection_name: &str) -> Result<Vec<MongoIndexInfo>, String> {
        let collection: Collection<Document> = self
            .client
            .database(&self.db_name)
            .collection(collection_name);

        let mut cursor = collection.list_indexes().await.map_err(|e| e.to_string())?;

        let mut indexes = Vec::new();

        while let Some(index) = cursor.try_next().await.map_err(|e| e.to_string())? {
            let name = index
                .options
                .as_ref()
                .and_then(|o| o.name.clone())
                .unwrap_or_else(|| "unknown".into());

            let unique = index
                .options
                .as_ref()
                .and_then(|o| o.unique)
                .unwrap_or(false);

            let sparse = index
                .options
                .as_ref()
                .and_then(|o| o.sparse)
                .unwrap_or(false);

            let hidden = index
                .options
                .as_ref()
                .and_then(|o| o.hidden)
                .unwrap_or(false);

            let ttl_seconds = index
                .options
                .as_ref()
                .and_then(|o| o.expire_after)
                .map(|duration| duration.as_secs() as i64);

            let keys = index
                .keys
                .iter()
                .map(|(field, value)| MongoIndexKey {
                    field: field.clone(),
                    order: match value {
                        Bson::Int32(v) if *v == -1 => MongoIndexOrder::Descending,

                        Bson::Int64(v) if *v == -1 => MongoIndexOrder::Descending,

                        Bson::Double(v) if *v == -1.0 => MongoIndexOrder::Descending,

                        _ => MongoIndexOrder::Ascending,
                    },
                })
                .collect();

            indexes.push(MongoIndexInfo {
                name,
                keys,
                unique,
                sparse,
                partial: false,
                ttl_seconds,
                hidden,
            });
        }

        Ok(indexes)
    }

    async fn fetch_validator(
        &self,
        collection_name: &str,
    ) -> Result<Option<MongoValidator>, String> {
        let result = self
            .client
            .database(&self.db_name)
            .run_command(doc! {
                "listCollections": 1,
                "filter": {
                    "name": collection_name
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let cursor = result.get_document("cursor").map_err(|e| e.to_string())?;

        let first = cursor
            .get_array("firstBatch")
            .map_err(|e| e.to_string())?
            .first();

        let Some(Bson::Document(collection)) = first else {
            return Ok(None);
        };

        let options = collection.get_document("options").ok();

        let Some(options) = options else {
            return Ok(None);
        };

        let validator = options.get_document("validator").ok().cloned();

        if validator.is_none() {
            return Ok(None);
        }

        Ok(Some(MongoValidator {
            expression: validator,
            validation_level: options.get_str("validationLevel").ok().map(str::to_string),

            validation_action: options.get_str("validationAction").ok().map(str::to_string),
        }))
    }

    // ============================================================
    // VIEWS
    // ============================================================

    async fn fetch_views(&self) -> Result<Vec<MongoViewInfo>, String> {
        let db = self.client.database(&self.db_name);

        let mut cursor = db.list_collections().await.map_err(|e| e.to_string())?;

        let mut views = Vec::new();

        while let Some(info) = cursor.try_next().await.map_err(|e| e.to_string())? {
            let options = &info.options;

            // Only process actual views
            let Some(source) = options.view_on.as_ref() else {
                continue;
            };

            let pipeline = options.pipeline.clone().unwrap_or_default();

            views.push(MongoViewInfo {
                name: info.name,
                source: source.to_string(),
                pipeline,
            });
        }

        Ok(views)
    }
}

#[async_trait]
impl DatabaseDriver for MongoDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();
        // use bson::doc;

        self.client
            .database(&self.db_name)
            .run_command(doc! {
                "ping": 1
            })
            .await
            .map_err(|e| e.to_string())?;

        let version_doc = self
            .client
            .database(&self.db_name)
            .run_command(doc! { "buildInfo": 1 })
            .await
            .map_err(|e| e.to_string())?;

        let version = version_doc
            .get_str("version")
            .unwrap_or("unknown")
            .to_string();

        Ok((
            format!("MongoDB {}", version),
            start.elapsed().as_millis() as u64,
        ))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let collections = self.fetch_collections().await?;
        let views = self.fetch_views().await?;

        Ok(DatabaseMetadata::Mongo(MongoSchemaInfo {
            database: MongoDatabaseInfo {
                name: self.db_name.clone(),
                stats: self.fetch_database_stats().await?,
            },
            collections,
            views,
        }))
    }
}

fn parse_server_version(raw: &str) -> ServerVersion {
    let mut parts = raw.split('.');

    let major = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    let minor = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    let patch = parts
        .next()
        .and_then(|v| v.split('-').next().and_then(|v| v.parse::<u32>().ok()))
        .unwrap_or(0);

    ServerVersion {
        major,
        minor,
        patch,
        raw: raw.to_string(),
    }
}
