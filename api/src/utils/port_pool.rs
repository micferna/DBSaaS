use std::collections::HashSet;
use std::sync::RwLock;

use crate::error::{AppError, AppResult};

pub struct PortPool {
    start: u16,
    end: u16,
    allocated: RwLock<HashSet<u16>>,
}

impl PortPool {
    pub fn new(start: u16, end: u16) -> Self {
        Self {
            start,
            end,
            allocated: RwLock::new(HashSet::new()),
        }
    }

    pub fn load_allocated(&self, ports: Vec<i32>) {
        let mut allocated = self.allocated.write().unwrap();
        for port in ports {
            if let Ok(p) = u16::try_from(port) {
                allocated.insert(p);
            }
        }
    }

    pub fn allocate(&self) -> AppResult<u16> {
        let mut allocated = self.allocated.write().unwrap();
        for port in self.start..=self.end {
            if !allocated.contains(&port) {
                allocated.insert(port);
                return Ok(port);
            }
        }
        Err(AppError::Internal("No available ports".to_string()))
    }

    pub fn release(&self, port: u16) {
        let mut allocated = self.allocated.write().unwrap();
        allocated.remove(&port);
    }
}
