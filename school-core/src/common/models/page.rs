use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u64,
    pub page_size: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
        }
    }
}

/// Generic pagination wrapper — the global contract for all list endpoints.
/// Every endpoint returning a collection MUST use Page<T>.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total_items: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total_items: i64, page: u64, page_size: u64) -> Self {
        let total_items = total_items.max(0) as u64;
        let total_pages = if page_size > 0 {
            (total_items as f64 / page_size as f64).ceil() as u64
        } else {
            0
        };

        Self {
            items,
            total_items,
            page,
            page_size,
            total_pages,
        }
    }

    pub fn empty(page: u64, page_size: u64) -> Self {
        Self {
            items: vec![],
            total_items: 0,
            page,
            page_size,
            total_pages: 0,
        }
    }
}
