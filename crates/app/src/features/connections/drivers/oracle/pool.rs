use oracle::Connection as OracleConnection;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};

pub struct OraclePool {
    username: String,
    password: String,
    connect_string: String,
    inner: Arc<PoolInner>,
}

struct PoolInner {
    idle: Mutex<Vec<OracleConnection>>,
    not_empty: Condvar,
    max: u32,
    /// Total connections ever opened (idle + currently checked out).
    /// Only ever incremented — connections are recycled, never closed,
    /// so this never needs to go back down.
    opened: Mutex<u32>,
}

impl OraclePool {
    pub async fn new(
        username: String,
        password: String,
        connect_string: String,
        min: u32,
        max: u32,
    ) -> Result<Self, String> {
        let max = max.max(min).max(1);
        let (u, p, c) = (username.clone(), password.clone(), connect_string.clone());
        let min_needed = min.max(1);

        let idle = tokio::task::spawn_blocking(move || -> Result<Vec<OracleConnection>, String> {
            let mut conns = Vec::with_capacity(min_needed as usize);
            for _ in 0..min_needed {
                conns.push(OracleConnection::connect(&u, &p, &c).map_err(|e| e.to_string())?);
            }
            Ok(conns)
        })
        .await
        .map_err(|e| e.to_string())??;

        let opened = idle.len() as u32;

        Ok(Self {
            username,
            password,
            connect_string,
            inner: Arc::new(PoolInner {
                idle: Mutex::new(idle),
                not_empty: Condvar::new(),
                max,
                opened: Mutex::new(opened),
            }),
        })
    }

    pub fn get(&self) -> Result<PooledOracleConnection, String> {
        let mut idle = self.inner.idle.lock().map_err(|e| e.to_string())?;

        loop {
            if let Some(conn) = idle.pop() {
                return Ok(PooledOracleConnection { conn: Some(conn), inner: Arc::clone(&self.inner) });
            }

            let mut opened = self.inner.opened.lock().map_err(|e| e.to_string())?;
            if *opened < self.inner.max {
                *opened += 1;
                drop(opened);
                drop(idle); // release before the blocking connect() call
                let conn = OracleConnection::connect(&self.username, &self.password, &self.connect_string)
                    .map_err(|e| e.to_string())?;
                return Ok(PooledOracleConnection { conn: Some(conn), inner: Arc::clone(&self.inner) });
            }
            drop(opened);

            // At `max` with none idle — block this (blocking-pool) thread
            // until a connection is returned. This is exactly what
            // spawn_blocking's dedicated thread pool exists to absorb.
            idle = self
                .inner
                .not_empty
                .wait(idle)
                .map_err(|_| "oracle pool lock poisoned".to_string())?;
        }
    }
}


pub struct PooledOracleConnection {
    conn: Option<OracleConnection>,
    inner: Arc<PoolInner>,
}

impl Deref for PooledOracleConnection {
    type Target = OracleConnection;
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("connection taken before drop — this should be unreachable")
    }
}

impl DerefMut for PooledOracleConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect("connection taken before drop — this should be unreachable")
    }
}

impl Drop for PooledOracleConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if let Ok(mut idle) = self.inner.idle.lock() {
                idle.push(conn);
                self.inner.not_empty.notify_one();
            }
        }
    }
}

// use oracle::Connection as OracleConnection;
// use std::sync::{Arc, Mutex as StdMutex};
// use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// use tokio::task;

// pub struct OraclePool {
//     username: String,
//     password: String,
//     connect_string: String,
//     idle: Arc<StdMutex<Vec<OracleConnection>>>,
//     semaphore: Arc<Semaphore>,
// }

// impl OraclePool {
//     pub async fn new(
//         username: String,
//         password: String,
//         connect_string: String,
//         min: u32,
//         max: u32,
//     ) -> Result<Self, String> {
//         let max = max.max(min).max(1);
//         let mut idle = Vec::with_capacity(min as usize);
//         for _ in 0..min.max(1) {
//             idle.push(Self::open(&username, &password, &connect_string).await?);
//         }

//         Ok(Self {
//             username,
//             password,
//             connect_string,
//             idle: Arc::new(StdMutex::new(idle)),
//             semaphore: Arc::new(Semaphore::new(max as usize)),
//         })
//     }

//     async fn open(
//         username: &str,
//         password: &str,
//         connect_string: &str,
//     ) -> Result<OracleConnection, String> {
//         let (u, p, c) = (
//             username.to_string(),
//             password.to_string(),
//             connect_string.to_string(),
//         );
//         task::spawn_blocking(move || OracleConnection::connect(&u, &p, &c))
//             .await
//             .map_err(|e| e.to_string())? // JoinError (panicked blocking task)
//             .map_err(|e| e.to_string()) // oracle::Error from connect()
//     }

//     pub async fn get(&self) -> Result<PooledOracleConnection, String> {
//         let permit = Arc::clone(&self.semaphore)
//             .acquire_owned()
//             .await
//             .map_err(|e| e.to_string())?;

//         let existing = {
//             let mut idle = self.idle.lock().map_err(|e| e.to_string())?;
//             idle.pop()
//         };

//         let conn = match existing {
//             Some(c) => c,
//             None => Self::open(&self.username, &self.password, &self.connect_string).await?,
//         };

//         Ok(PooledOracleConnection {
//             conn: Some(conn),
//             idle: Arc::clone(&self.idle),
//             _permit: permit,
//         })
//     }
// }

// pub struct PooledOracleConnection {
//     conn: Option<OracleConnection>,
//     idle: Arc<StdMutex<Vec<OracleConnection>>>,
//     _permit: OwnedSemaphorePermit,
// }

// impl PooledOracleConnection {
//     pub async fn run_blocking<F, T>(&mut self, f: F) -> Result<T, String>
//     where
//         F: FnOnce(&mut OracleConnection) -> Result<T, String> + Send + 'static,
//         T: Send + 'static,
//     {
//         let mut conn = self
//             .conn
//             .take()
//             .expect("run_blocking called on a connection already checked out — did a previous call not return it?");

//         let (result, conn) = task::spawn_blocking(move || {
//             let result = f(&mut conn);
//             (result, conn)
//         })
//         .await
//         .map_err(|e| e.to_string())?;

//         self.conn = Some(conn);
//         result
//     }
// }

// impl Drop for PooledOracleConnection {
//     fn drop(&mut self) {
//         if let Some(conn) = self.conn.take() {
//             if let Ok(mut idle) = self.idle.lock() {
//                 idle.push(conn);
//             }
//         }
//     }
// }
