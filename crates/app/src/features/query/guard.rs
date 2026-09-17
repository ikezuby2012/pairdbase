use sqlparser::{
    ast::Statement,
    dialect::{GenericDialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect},
    parser::Parser,
};

use super::domain::{ConnectionPermission, GuardVerdict, RejectionReason, StatementType};
use crate::features::connections::domain::DbType;

pub fn evaluate(query: &str, db_type: &DbType, permission: &ConnectionPermission) -> GuardVerdict {
    // MongoDB and Redis have no SQL — pass directly through the guard
    // They are controlled only by the permission check at the use_case level
    if matches!(db_type, DbType::MongoDB | DbType::Redis) {
        return GuardVerdict::Approved {
            statement_type: StatementType::Other,
        };
    }

    // Stage 1: parse
    let statements = match parse(query, db_type) {
        Ok(s) if s.is_empty() => {
            return GuardVerdict::Rejected {
                reason: RejectionReason::ParseFailure {
                    detail: "empty query".into(),
                },
            }
        }
        Ok(s) => s,
        Err(e) => {
            return GuardVerdict::Rejected {
                reason: RejectionReason::ParseFailure { detail: e },
            }
        }
    };

    let stmt = &statements[0];
    let stmt_type = classify(stmt);

    // Stage 2: permission
    if let Some(reason) = check_permission(&stmt_type, permission) {
        return GuardVerdict::Rejected { reason };
    }

    // Stage 3a: unbounded mutation
    if let Some(reason) = check_unbounded_mutation(stmt, &stmt_type) {
        return GuardVerdict::Rejected { reason };
    }

    // Stage 3b: injection
    if let Some(reason) = check_injection(query) {
        return GuardVerdict::Rejected { reason };
    }

    GuardVerdict::Approved {
        statement_type: stmt_type,
    }
}

fn parse(query: &str, db_type: &DbType) -> Result<Vec<Statement>, String> {
    let result = match db_type {
        DbType::PostgreSQL => Parser::parse_sql(&PostgreSqlDialect {}, query),
        DbType::MySQL => Parser::parse_sql(&MySqlDialect {}, query),
        DbType::SqlServer => Parser::parse_sql(&MsSqlDialect {}, query),
        // Oracle uses generic — sqlparser has no dedicated Oracle dialect
        _ => Parser::parse_sql(&GenericDialect {}, query),
    };
    result.map_err(|e| e.to_string())
}

fn classify(stmt: &Statement) -> StatementType {
    match stmt {
        Statement::Query(_) => StatementType::Select,
        Statement::Insert(_) => StatementType::Insert,
        Statement::Update(_) => StatementType::Update,
        Statement::Delete(_) => StatementType::Delete,
        Statement::CreateTable(_)
        | Statement::CreateIndex(_)
        | Statement::CreateView(_)
        | Statement::AlterTable(_)
        | Statement::Drop { .. }
        | Statement::Truncate { .. } => StatementType::Ddl,
        _ => StatementType::Other,
    }
}

fn check_permission(
    stmt_type: &StatementType,
    permission: &ConnectionPermission,
) -> Option<RejectionReason> {
    let allowed = match stmt_type {
        StatementType::Select => permission.allow_select,
        StatementType::Insert => permission.allow_insert,
        StatementType::Update => permission.allow_update,
        StatementType::Delete => permission.allow_delete,
        StatementType::Ddl => permission.allow_ddl,
        StatementType::Other => true,
    };

    if !allowed {
        Some(RejectionReason::InsufficientPermission {
            statement_type: stmt_type.clone(),
        })
    } else {
        None
    }
}

fn check_unbounded_mutation(
    stmt: &Statement,
    stmt_type: &StatementType,
) -> Option<RejectionReason> {
    let unbounded = match stmt {
        Statement::Update(u) => u.selection.is_none(),
        Statement::Delete(d) => d.selection.is_none(),
        _ => false,
    };

    if unbounded {
        Some(RejectionReason::UnboundedMutation {
            statement_type: stmt_type.clone(),
        })
    } else {
        None
    }
}

fn check_injection(query: &str) -> Option<RejectionReason> {
    let n = query
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let patterns: &[(&str, &str)] = &[
        ("; drop ", "stacked DROP"),
        ("; delete ", "stacked DELETE"),
        ("; insert ", "stacked INSERT"),
        ("; update ", "stacked UPDATE"),
        ("; truncate ", "stacked TRUNCATE"),
        ("or 1=1", "tautology OR 1=1"),
        ("or '1'='1'", "tautology OR '1'='1'"),
        ("pg_sleep(", "time-delay: pg_sleep"),
        ("sleep(", "time-delay: sleep()"),
        ("waitfor delay", "time-delay: WAITFOR DELAY"),
        ("dbms_pipe", "Oracle pipe injection"),
        ("utl_http", "Oracle UTL_HTTP exfil"),
        ("xp_cmdshell", "OS command: xp_cmdshell"),
        ("load_file(", "file read: LOAD_FILE"),
        ("into outfile", "file write: INTO OUTFILE"),
        ("information_schema.tables", "schema enumeration"),
        ("pg_catalog.pg_user", "user enumeration"),
        ("sys.sql_logins", "SQL Server login enum"),
        ("all_users", "Oracle user enumeration"),
    ];

    for (pattern, desc) in patterns {
        if n.contains(pattern) {
            return Some(RejectionReason::InjectionSuspect {
                detail: desc.to_string(),
            });
        }
    }

    None
}

