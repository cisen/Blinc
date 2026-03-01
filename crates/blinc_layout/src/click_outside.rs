//! Click-outside detection system
//!
//! Components (like select dropdowns) can register to be notified when a click
//! occurs outside their subtree. The event router calls `fire_click_outside()`
//! on every mouse down, passing the hit target's ancestor element IDs.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

type DismissCallback = Arc<dyn Fn() + Send + Sync>;

struct ClickOutsideEntry {
    /// The element ID that defines "inside". If this ID appears in the
    /// hit target's ancestor element IDs, the click is considered inside.
    element_id: String,
    /// Called when a click occurs outside the element's subtree.
    on_dismiss: DismissCallback,
}

static REGISTRY: LazyLock<Mutex<HashMap<String, ClickOutsideEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a click-outside handler.
///
/// - `key`: Unique key for this registration (used to unregister).
/// - `element_id`: The element ID in the DOM tree. Clicks whose ancestor chain
///   does NOT include this ID trigger the dismiss callback.
/// - `on_dismiss`: Callback to invoke on click-outside.
pub fn register_click_outside(
    key: &str,
    element_id: &str,
    on_dismiss: impl Fn() + Send + Sync + 'static,
) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.insert(
            key.to_string(),
            ClickOutsideEntry {
                element_id: element_id.to_string(),
                on_dismiss: Arc::new(on_dismiss),
            },
        );
    }
}

/// Unregister a click-outside handler.
pub fn unregister_click_outside(key: &str) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.remove(key);
    }
}

/// Fire click-outside callbacks for a click event.
///
/// Called by the event router on every mouse down.
/// `ancestor_element_ids` contains the element IDs of nodes in the hit target's ancestor chain.
/// If empty (click on empty space), all handlers fire.
pub fn fire_click_outside(ancestor_element_ids: &[String]) {
    let callbacks: Vec<(String, DismissCallback)> = {
        let Ok(reg) = REGISTRY.lock() else {
            return;
        };
        if reg.is_empty() {
            return;
        }
        reg.iter()
            .filter(|(_, entry)| {
                !ancestor_element_ids
                    .iter()
                    .any(|id| *id == entry.element_id)
            })
            .map(|(key, entry)| (key.clone(), Arc::clone(&entry.on_dismiss)))
            .collect()
    };

    if !callbacks.is_empty() {
        let keys: Vec<_> = callbacks.iter().map(|(k, _)| k.as_str()).collect();
        eprintln!(
            "[CLICK_OUTSIDE] Firing {} handlers: {:?} (ancestors: {:?})",
            callbacks.len(),
            keys,
            ancestor_element_ids
        );
    }

    for (_, cb) in callbacks {
        cb();
    }
}

/// Clear all registrations.
pub fn clear_click_outside_handlers() {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.clear();
    }
}
