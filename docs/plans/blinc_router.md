# blinc_router — Cross-Platform Routing System

## Overview

A new crate `blinc_router` providing declarative routing with deep linking, platform-native URI handling, FSM-driven navigation state, and spring-animated page transitions. Built on top of `blinc_core` (Store, reactive signals, events) and `blinc_platform` (cross-platform deep link trait).

**Crate location:** `crates/blinc_router/`

---

## 1. Crate Structure

```text
crates/blinc_router/
├── Cargo.toml
└── src/
    ├── lib.rs            # Re-exports, Router builder, global router access
    ├── route.rs          # Route definition, path matching, parameters
    ├── history.rs        # History stack, Store<RouterHistory> integration
    ├── navigator.rs      # Navigation actions (push, pop, replace, deep link)
    ├── state.rs          # RouterState FSM (StateTransitions impl)
    ├── events.rs         # Navigation event type constants
    ├── deep_link.rs      # URI parsing, platform deep link trait
    ├── transition.rs     # Page transition config (spring/easing presets)
    └── middleware.rs      # Navigation guards / middleware hooks
```

### Cargo.toml

```toml
[package]
name = "blinc_router"
description = "Cross-platform routing with deep linking for Blinc UI framework"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
keywords = ["ui", "gui", "router", "navigation", "blinc"]

[dependencies]
blinc_core = { path = "../blinc_core", version = "0.1.12" }
blinc_animation = { path = "../blinc_animation", version = "0.1.12" }
tracing.workspace = true
rustc-hash.workspace = true
smallvec.workspace = true
```

No dependency on `blinc_platform` — the deep link trait lives in `blinc_platform` and platform implementations live in `blinc_platform_desktop/android/ios`. The router only defines types that those implementations consume.

---

## 2. Route Definition (`route.rs`)

### Route Tree

```rust
pub struct RouteConfig {
    pub path: &'static str,            // "/users/:id/posts"
    pub name: Option<&'static str>,    // "user-posts" (for named navigation)
    pub builder: RouteBuilder,         // fn(RouteContext) -> Div
    pub children: Vec<RouteConfig>,    // Nested routes
    pub transition: Option<PageTransition>,
    pub guards: Vec<NavigationGuard>,
}
```

### Path Syntax

| Pattern | Example | Matches |
| ------- | ------- | ------- |
| Static | `/about` | Exact match |
| Parameter | `/users/:id` | `/users/42` → `{id: "42"}` |
| Optional | `/users/:id?` | `/users` and `/users/42` |
| Wildcard | `/files/*path` | `/files/a/b/c` → `{path: "a/b/c"}` |
| Nested | parent + child | `/dashboard` + `/settings` → `/dashboard/settings` |

### Path Matching

```rust
pub struct MatchedRoute {
    pub config: &'static RouteConfig,
    pub params: RouteParams,            // FxHashMap<String, String>
    pub query: QueryParams,             // FxHashMap<String, String>
    pub matched_path: String,           // The resolved path
    pub nested: Vec<MatchedRoute>,      // Matched child routes
}

pub struct RouteParams(FxHashMap<String, String>);

impl RouteParams {
    pub fn get(&self, key: &str) -> Option<&str>;
    pub fn get_parsed<T: FromStr>(&self, key: &str) -> Option<T>;
}
```

Matching algorithm: Trie-based prefix tree built at router initialization. Static segments have priority over parameters, parameters over wildcards. This runs once at `Router::new()` — the trie is immutable after creation.

### Builder API

```rust
use blinc_router::{Route, Router};

Router::new()
    .route(Route::new("/")
        .name("home")
        .view(home_page))
    .route(Route::new("/users")
        .name("users")
        .view(users_list)
        .child(Route::new("/:id")
            .name("user-detail")
            .view(user_detail)
            .child(Route::new("/posts")
                .view(user_posts))))
    .route(Route::new("/settings")
        .view(settings_page)
        .guard(require_auth))
    .not_found(not_found_page)
    .build()
```

### Route View Signature

```rust
pub type RouteBuilder = fn(RouteContext) -> Div;

pub struct RouteContext {
    pub params: RouteParams,
    pub query: QueryParams,
    pub path: String,
    pub router: RouterHandle,  // For programmatic navigation
}
```

---

## 3. History Management (`history.rs`)

Uses `Store<RouterHistory>` from `blinc_core::store` for persistent, subscriber-aware history.

```rust
pub struct RouterHistory {
    pub back_stack: Vec<HistoryEntry>,
    pub forward_stack: Vec<HistoryEntry>,
    pub current: HistoryEntry,
    pub max_size: usize,               // Default 50
}

pub struct HistoryEntry {
    pub path: String,
    pub params: RouteParams,
    pub query: QueryParams,
    pub state: Option<Box<dyn Any + Send + Sync>>,  // Arbitrary per-route state
    pub timestamp: u64,
    pub title: Option<String>,
}
```

### Store Integration

```rust
// Global router history store — created once at Router::build()
let store = create_store::<RouterHistory>("blinc_router");
store.set("history", RouterHistory::new("/"));

// Navigation pushes entries
store.update("history", |h| {
    h.forward_stack.clear();
    h.back_stack.push(h.current.clone());
    if h.back_stack.len() > h.max_size {
        h.back_stack.remove(0);
    }
    h.current = new_entry;
});

// Subscribers get notified on every navigation
store.subscribe("history", |history| {
    // Update reactive signal, trigger UI rebuild
    current_route_signal.set(history.current.clone());
});
```

### History Operations

```rust
impl RouterHistory {
    pub fn push(&mut self, entry: HistoryEntry);
    pub fn replace(&mut self, entry: HistoryEntry);  // No back stack change
    pub fn back(&mut self) -> Option<HistoryEntry>;
    pub fn forward(&mut self) -> Option<HistoryEntry>;
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
    pub fn clear(&mut self);
}
```

---

## 4. Navigation Events (`events.rs`)

New event type constants registered alongside existing events in `blinc_core::events`.

```rust
// Navigation event types (range 100-119, avoiding existing ranges)
pub const NAVIGATE: EventType = 100;        // Push navigation
pub const NAVIGATE_REPLACE: EventType = 101; // Replace navigation
pub const NAVIGATE_BACK: EventType = 102;    // Pop back
pub const NAVIGATE_FORWARD: EventType = 103; // Go forward
pub const DEEP_LINK: EventType = 104;        // Incoming deep link URI
pub const ROUTE_CHANGED: EventType = 105;    // After route change (for observers)
pub const ROUTE_GUARD_REJECT: EventType = 106; // Guard blocked navigation
```

### Event Data

```rust
pub enum NavigationEventData {
    Navigate { path: String, state: Option<Box<dyn Any + Send + Sync>> },
    Back,
    Forward,
    DeepLink { uri: String, source: DeepLinkSource },
    RouteChanged { from: String, to: String, params: RouteParams },
}

pub enum DeepLinkSource {
    System,     // OS-level deep link (Android intent, iOS universal link)
    Internal,   // In-app programmatic deep link
    Push,       // Push notification payload
}
```

---

## 5. Page Stack Architecture

The router does **not** use a reactive signal that swaps a single outlet's content. Instead, it maintains a **page stack** — a `stack()` container where each route's page is a layer. Pushing a route adds a new layer on top; popping removes the top layer. Only the topmost page is "active" — all pages below are **suspended** (animations paused, state signals frozen, input disabled).

This model naturally enables entry/exit animations: a pushed page animates in from off-screen (or fades in), and a popped page animates out before being removed from the stack.

### Page Lifecycle

```text
                push("/users")                    back()
                     │                              │
   ┌─────────┐      ▼      ┌─────────┐            ▼      ┌─────────┐
   │  Home   │  ──────►    │  Users  │ ◄─ active  ───►   │  Home   │ ◄─ active
   │ (active)│             │ (enter) │            (exit)  │(resume) │
   └─────────┘             ├─────────┤                    └─────────┘
                           │  Home   │ ◄─ suspended
                           │(suspend)│
                           └─────────┘
```

### Page States

Each page in the stack has a lifecycle state, also driven by the FSM:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PageState {
    Entering,      // Page is animating in (entry transition playing)
    Active,        // Page is visible, interactive, animations running
    Suspended,     // Page is in stack but not visible — animations/signals frozen
    Exiting,       // Page is animating out (exit transition playing)
    Removed,       // Exit animation complete — remove from stack on next frame
}
```

### Router State FSM (`state.rs`)

Implements `StateTransitions` for FSM-driven navigation transient states.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RouterState {
    Idle,              // Top page is Active, no transition in progress
    Pushing,           // New page entering, current page suspending
    Popping,           // Top page exiting, page below resuming
    Replacing,         // Top page exiting, new page entering simultaneously
    Error,             // Navigation failed (guard rejected, route not found)
}

impl StateTransitions for RouterState {
    fn on_event(&self, event: u32) -> Option<Self> {
        match (self, event) {
            // Push navigation
            (RouterState::Idle, NAVIGATE | DEEP_LINK) => {
                Some(RouterState::Pushing)
            }
            // Replace navigation
            (RouterState::Idle, NAVIGATE_REPLACE) => {
                Some(RouterState::Replacing)
            }
            // Back/forward = pop
            (RouterState::Idle, NAVIGATE_BACK | NAVIGATE_FORWARD) => {
                Some(RouterState::Popping)
            }
            // Transition complete → idle
            (RouterState::Pushing | RouterState::Popping | RouterState::Replacing,
             ROUTE_CHANGED) => {
                Some(RouterState::Idle)
            }
            // Guard rejected
            (RouterState::Pushing | RouterState::Replacing, ROUTE_GUARD_REJECT) => {
                Some(RouterState::Error)
            }
            // Error recovery
            (RouterState::Error, NAVIGATE | NAVIGATE_BACK) => {
                Some(RouterState::Idle)
            }
            _ => None,
        }
    }
}
```

### Page Suspension

When a page transitions from Active → Suspended:

1. **Animations paused**: All `AnimatedValue` springs and CSS animations on the page subtree are frozen. Leverages the existing `motion_is_suspended` flag — set `true` on the page's root motion container.
2. **State signals frozen**: The page's `stateful` callbacks stop receiving dependency notifications. Signals still hold their values but `.set()` calls are buffered, not applied.
3. **Input disabled**: The suspended page's `pointer_events_none` flag is set on its stack layer, so all input passes through to the active page above.
4. **Rendering skipped**: Suspended pages below the active page can optionally skip rendering entirely (not in the paint call). Since `stack()` clips by default and the active page covers the full area, the GPU work is naturally eliminated.

When a page resumes (Active again after the page above is popped):

1. **Animations resume**: Frozen springs continue from their paused values. CSS animations resume with adjusted timestamps.
2. **State signals flush**: Buffered signal updates are applied, triggering a single rebuild with the latest values.
3. **Input re-enabled**: `pointer_events_none` cleared.

### Reactive Helpers

```rust
/// Get the current active route's params (reads from top of page stack)
pub fn use_route() -> MatchedRoute;

/// Get route params (convenience)
pub fn use_params() -> RouteParams;

/// Get query params (convenience)
pub fn use_query() -> QueryParams;

/// Get router handle (for programmatic navigation)
pub fn use_router() -> RouterHandle;

/// Check if the current page is the active (top) page
/// Useful for components that behave differently when suspended
pub fn use_is_active() -> bool;
```

---

## 6. Navigator (`navigator.rs`)

The `RouterHandle` is the public API for programmatic navigation.

```rust
#[derive(Clone)]
pub struct RouterHandle {
    history_store: &'static Store<RouterHistory>,
    route_signal: State<MatchedRoute>,
    router_state: State<RouterState>,
    route_trie: Arc<RouteTrie>,
    guards: Arc<Vec<NavigationGuard>>,
    transition_config: Arc<TransitionConfig>,
}

impl RouterHandle {
    /// Navigate to a path
    pub fn push(&self, path: impl Into<String>);

    /// Navigate to a named route with params
    pub fn push_named(&self, name: &str, params: &[(&str, &str)]);

    /// Replace current route (no back stack entry)
    pub fn replace(&self, path: impl Into<String>);

    /// Go back
    pub fn back(&self);

    /// Go forward
    pub fn forward(&self);

    /// Check if back is possible
    pub fn can_go_back(&self) -> bool;

    /// Check if forward is possible
    pub fn can_go_forward(&self) -> bool;

    /// Navigate from deep link URI
    pub fn handle_deep_link(&self, uri: &str, source: DeepLinkSource);

    /// Get current path
    pub fn current_path(&self) -> String;
}
```

### Push Flow (navigate forward)

```text
router.push("/users/42")
  │
  ├─ 1. Match route in trie → MatchedRoute { config, params: {id: "42"}, ... }
  │     If no match → not_found route
  │
  ├─ 2. Run guards sequentially
  │     guard(from, to) → GuardResult::Allow | Redirect(path) | Reject(reason)
  │     If rejected → dispatch ROUTE_GUARD_REJECT, RouterState → Error
  │     If redirect → restart with new path
  │
  ├─ 3. Suspend current top page
  │     PageState: Active → Suspended
  │     Freeze animations, buffer signal updates, set pointer_events_none
  │
  ├─ 4. Build new page & push onto stack
  │     Call (config.builder)(RouteContext { params, ... }) → Div
  │     Wrap in stack layer with entry transition (e.g. off-screen right)
  │     PageState: Entering
  │
  ├─ 5. Update history store
  │     store.update("history", |h| h.push(new_entry))
  │
  ├─ 6. Play entry animation
  │     New page animates in (slide from right, fade in, scale up, etc.)
  │     On animation complete: PageState: Entering → Active
  │     Dispatch ROUTE_CHANGED, RouterState: Pushing → Idle
  │
  └─ Stack state:  [ Home (suspended) | Users (active) ]
```

### Pop Flow (navigate back)

```text
router.back()
  │
  ├─ 1. Start exit animation on top page
  │     PageState: Active → Exiting
  │     Play exit transition (slide out right, fade out, etc.)
  │
  ├─ 2. Resume page below (while exit animates)
  │     PageState: Suspended → Active
  │     Flush buffered signals, resume animations, re-enable input
  │
  ├─ 3. On exit animation complete
  │     PageState: Exiting → Removed
  │     Remove page Div from stack, drop all page state
  │
  ├─ 4. Update history store
  │     store.update("history", |h| h.back())
  │
  ├─ 5. Dispatch ROUTE_CHANGED
  │     RouterState: Popping → Idle
  │
  └─ Stack state:  [ Home (active) ]
```

---

## 7. Deep Linking (`deep_link.rs`)

### URI Parsing

```rust
pub struct DeepLink {
    pub scheme: String,        // "myapp", "https"
    pub host: Option<String>,  // "example.com"
    pub path: String,          // "/users/42"
    pub query: QueryParams,    // ?tab=posts → {tab: "posts"}
    pub fragment: Option<String>, // #section
}

impl DeepLink {
    /// Parse a URI string into a DeepLink
    pub fn parse(uri: &str) -> Result<Self, DeepLinkError>;

    /// Convert to a router-compatible path string
    pub fn to_route_path(&self) -> String;
}
```

### Platform Deep Link Trait (added to `blinc_platform`)

```rust
// In blinc_platform/src/deep_link.rs (new file)

/// Platform-specific deep link handling
pub trait DeepLinkHandler: Send + Sync {
    /// Called when the app receives a deep link from the OS
    fn on_deep_link(&self, uri: &str);

    /// Register URL schemes this app handles
    fn registered_schemes(&self) -> &[&str];

    /// Get the launch URI if the app was opened via deep link
    fn launch_uri(&self) -> Option<String>;
}
```

### Platform Implementations

**Android** (`blinc_platform_android`):

- Read intent URI from `Activity.getIntent().getData()` via JNI
- `on_deep_link()` called from `onNewIntent()` JNI callback
- Registered via `AndroidManifest.xml` intent filters (app responsibility)

**iOS** (`blinc_platform_ios`):

- Read URL from `UIApplicationDelegate.application(_:open:options:)`
- Universal Links from `application(_:continue:restorationHandler:)`
- Called via Objective-C callback bridge

**Desktop** (`blinc_platform_desktop`):

- Custom URL scheme registration (OS-specific)
- Single-instance IPC: second launch sends URI to running instance
- Command-line argument: `--deep-link=myapp://path`

### Integration with Router

```rust
// In blinc_app runner initialization:
if let Some(launch_uri) = platform.deep_link_handler().launch_uri() {
    router.handle_deep_link(&launch_uri, DeepLinkSource::System);
}

// Register for runtime deep links
platform.deep_link_handler().set_callback(|uri| {
    router.handle_deep_link(uri, DeepLinkSource::System);
});
```

---

## 8. Page Transitions (`transition.rs`)

Since pages live on a stack, transitions are naturally **entry** and **exit** animations. Each route can define its own transition, and the router automatically plays the correct direction on push vs pop.

```rust
pub struct PageTransition {
    /// Animation when this page is pushed onto the stack
    pub enter: TransitionEffect,
    /// Animation when this page is popped off the stack
    pub exit: TransitionEffect,
    /// Spring config (overrides duration if set)
    pub spring: Option<SpringConfig>,
    /// Duration for easing-based transitions (ignored if spring is set)
    pub duration_ms: u32,
    /// Easing function (used when spring is None)
    pub easing: Easing,
}

pub enum TransitionEffect {
    /// Slide in from / out to a direction
    Slide(SlideDirection),
    /// Fade opacity
    Fade,
    /// Scale from/to a factor
    Scale { from: f32, to: f32 },
    /// Combined: slide + fade simultaneously
    SlideFade(SlideDirection),
    /// No animation (instant)
    None,
    /// Custom: receives progress 0.0→1.0, returns (translate_x, translate_y, opacity, scale)
    Custom(fn(f32) -> (f32, f32, f32, f32)),
}

pub enum SlideDirection {
    Left,     // Enter: from right → center. Exit: center → left.
    Right,    // Enter: from left → center. Exit: center → right.
    Up,       // Enter: from bottom → center. Exit: center → up.
    Down,     // Enter: from top → center. Exit: center → down.
}
```

### Presets

```rust
impl PageTransition {
    /// iOS-style: push slides in from right, pop slides out to right
    pub fn slide() -> Self;

    /// Crossfade between pages
    pub fn fade() -> Self;

    /// Modal: slides up from bottom, dismisses down
    pub fn modal() -> Self;

    /// Slide + fade combined (Material-style)
    pub fn slide_fade() -> Self;

    /// No animation — instant swap
    pub fn none() -> Self;

    /// Builder: override spring
    pub fn with_spring(self, spring: SpringConfig) -> Self;

    /// Builder: override duration
    pub fn with_duration(self, ms: u32) -> Self;
}
```

### How Transitions Map to the Stack

Each page in the `stack()` is wrapped in a transition container div. The router applies transforms/opacity to this wrapper:

```text
Push "/users" with PageTransition::slide():
  1. New page layer added to stack() at top
  2. Wrapper starts at Transform::translate(screen_width, 0), opacity 1.0
  3. Spring/easing animates translate_x from screen_width → 0
  4. Page below is already suspended (no exit animation needed — it stays in place)

Pop (back) from "/users":
  1. Top page wrapper starts at Transform::translate(0, 0)
  2. Spring/easing animates translate_x from 0 → screen_width
  3. Page below resumes immediately (visible underneath as top slides away)
  4. On animation complete → remove top page layer from stack

Replace "/settings" → "/profile":
  1. New page enters with enter transition
  2. Old page exits with exit transition (simultaneous)
  3. On both complete → remove old page from stack
```

The wrapper div uses `motion()` with `motion_is_suspended` to coordinate animation start, and the existing `motion_on_ready_callback` to begin the entry animation only after the page is laid out.

---

## 9. Navigation Guards (`middleware.rs`)

```rust
pub type NavigationGuard = Arc<dyn Fn(&HistoryEntry, &MatchedRoute) -> GuardResult + Send + Sync>;

pub enum GuardResult {
    Allow,
    Redirect(String),        // Redirect to a different path
    Reject(String),          // Block with reason
}

// Example: auth guard
fn require_auth(from: &HistoryEntry, to: &MatchedRoute) -> GuardResult {
    if is_authenticated() {
        GuardResult::Allow
    } else {
        GuardResult::Redirect("/login".into())
    }
}
```

Guards run sequentially before navigation commits. First non-Allow result wins.

---

## 10. Route Stack Widget (for `blinc_app` integration)

The router provides a `route_stack()` widget — a `stack()` container that manages all pages in the navigation stack. Pages are real Div subtrees that persist in the stack, not rebuilt on every navigation.

### Internal Structure

```rust
/// The page stack — place this in your layout where route content should appear
pub fn route_stack() -> Div {
    let router = use_router_internal();
    let pages = router.page_stack.get(); // Vec<PageEntry>

    let mut page_stack = stack().w_full().h_full();

    for (i, page) in pages.iter().enumerate() {
        let is_top = i == pages.len() - 1;
        let wrapper = page_transition_wrapper(page, is_top);
        page_stack = page_stack.child(wrapper);
    }

    page_stack
}

/// Each page is wrapped in a transition container
fn page_transition_wrapper(page: &PageEntry, is_top: bool) -> Div {
    let mut wrapper = div()
        .w_full()
        .h_full()
        .child(page.content.clone());

    // Suspended pages: no input, skip animations
    if !is_top {
        wrapper = wrapper.pointer_events_none();
        // motion_is_suspended propagates to all child animations
    }

    // Apply transition transform/opacity during Entering/Exiting states
    match page.state {
        PageState::Entering => {
            wrapper = wrapper.transform(page.current_enter_transform());
            wrapper = wrapper.opacity(page.current_enter_opacity());
        }
        PageState::Exiting => {
            wrapper = wrapper.transform(page.current_exit_transform());
            wrapper = wrapper.opacity(page.current_exit_opacity());
        }
        PageState::Active => { /* identity transform, full opacity */ }
        PageState::Suspended => { /* kept in tree but covered by pages above */ }
        PageState::Removed => unreachable!(), // cleaned up before render
    }

    wrapper
}
```

### PageEntry

```rust
pub struct PageEntry {
    pub route: MatchedRoute,
    pub content: Div,                       // The built page Div (persisted)
    pub state: PageState,
    pub transition: PageTransition,
    pub enter_progress: AnimatedValue,      // 0.0 → 1.0 spring/easing
    pub exit_progress: AnimatedValue,       // 0.0 → 1.0 spring/easing
    pub suspended_signals: Vec<SignalId>,   // Signals to freeze/flush
}
```

### Nested Route Stacks

For layout routes (e.g. `/dashboard` with sub-routes), the layout page contains its own `route_stack()` scoped to its children:

```rust
// Layout route at /dashboard
fn dashboard_layout(ctx: RouteContext) -> Div {
    div().flex_row()
        .child(sidebar())
        .child(route_stack())  // Nested stack for /dashboard/* sub-routes
}
```

Each nested `route_stack()` manages its own page stack independently. The parent dashboard page stays Active while child pages push/pop within the nested stack.

---

## 11. Full Usage Example

```rust
use blinc_router::{Router, Route, PageTransition, use_router, route_stack};

fn main() {
    let router = Router::new()
        .route(Route::new("/")
            .name("home")
            .view(home_page))
        .route(Route::new("/users")
            .name("users")
            .view(users_page)
            .transition(PageTransition::slide())
            .child(Route::new("/:id")
                .name("user-detail")
                .view(user_detail_page)
                .transition(PageTransition::slide_fade())))
        .route(Route::new("/settings")
            .view(settings_page)
            .transition(PageTransition::modal())   // Slides up like a modal
            .guard(require_auth))
        .not_found(not_found_page)
        .build();

    WindowedApp::new("My App", 900, 700)
        .with_router(router)
        .run(build_ui);
}

fn build_ui(ctx: &mut BlincContext) -> Div {
    div().flex_col().w_full().h_full()
        .child(nav_bar())
        .child(route_stack())  // Page stack renders here — pages push/pop with animations
}

fn nav_bar() -> Div {
    let router = use_router();
    div().flex_row().h(48.0).bg(Color::from_hex("#1a1a2e"))
        .child(
            button("Home").on_click({
                let r = router.clone();
                move |_| r.push("/")
            })
        )
        .child(
            button("Users").on_click({
                let r = router.clone();
                move |_| r.push("/users")
            })
        )
        .child(
            button("Back").on_click({
                let r = router.clone();
                move |_| r.back()  // Pops top page with exit animation
            })
        )
}

fn home_page(ctx: RouteContext) -> Div {
    // This page persists in the stack when /users is pushed on top.
    // Animations here freeze while suspended, resume when popped back to.
    div()
        .child(text("Welcome home"))
        .child(animated_content())  // Springs/CSS animations pause when suspended
}

fn user_detail_page(ctx: RouteContext) -> Div {
    let id = ctx.params.get_parsed::<u64>("id").unwrap_or(0);
    let router = ctx.router.clone();

    div()
        .child(text(&format!("User #{}", id)))
        .child(
            button("Back")
                .on_click(move |_| router.back())  // Plays exit animation, reveals users_page below
        )
}
```

### What Happens at Runtime

```text
1. App starts → route_stack() contains [ Home (active) ]
2. User clicks "Users" →
   - Home page suspends (animations freeze, input disabled)
   - Users page pushes onto stack, slides in from right
   - Stack: [ Home (suspended) | Users (active) ]
3. User taps a user → router.push("/users/42")
   - Users page suspends
   - UserDetail page pushes, slide-fades in
   - Stack: [ Home (suspended) | Users (suspended) | UserDetail (active) ]
4. User clicks "Back" →
   - UserDetail plays exit animation (slide-fades out)
   - Users page resumes (animations restart from where they froze)
   - On exit complete, UserDetail is removed from stack
   - Stack: [ Home (suspended) | Users (active) ]
```

---

## 12. Implementation Order

### Phase 1: Core routing + page stack (no transitions, no deep links)

1. Create crate skeleton, register in workspace
2. `route.rs` — Route definition, path parsing, trie-based matching
3. `history.rs` — RouterHistory with Store integration
4. `navigator.rs` — RouterHandle with push/replace/back/forward
5. `lib.rs` — Router builder, global access (use_route, use_router), PageEntry
6. `route_stack()` widget — stack()-based page container with push/pop
7. Basic suspension: pointer_events_none on non-top pages
8. Basic example app (instant page swaps, no animations)

### Phase 2: FSM + Entry/Exit Animations

1. `events.rs` — Navigation event constants
2. `state.rs` — RouterState + PageState FSMs implementing StateTransitions
3. `transition.rs` — PageTransition presets (slide, fade, modal, slide_fade)
4. Wire AnimatedValue springs into PageEntry for enter/exit progress
5. Transition wrapper in route_stack() — apply transforms based on PageState
6. Cleanup: remove PageState::Removed entries after exit animation completes

### Phase 3: Suspension System

1. Animation suspension: propagate motion_is_suspended to page subtrees
2. Signal buffering: buffer .set() calls on suspended pages, flush on resume
3. Animation resume: continue springs from paused values with adjusted timestamps
4. Optional render skip for fully-covered suspended pages

### Phase 4: Guards + Deep Linking

1. `middleware.rs` — Navigation guards
2. `deep_link.rs` — URI parsing
3. Add `DeepLinkHandler` trait to `blinc_platform`
4. Implement in `blinc_platform_desktop` (CLI arg + single-instance)
5. Implement in `blinc_platform_android` (intent URI via JNI)
6. Implement in `blinc_platform_ios` (URL scheme + universal links)
7. Wire into `blinc_app` runners (windowed.rs, android.rs, ios.rs)

### Phase 5: Polish

1. Named route navigation (reverse lookup from name → path template)
2. Nested route stacks (scoped sub-stacks for layout routes)
3. Query parameter helpers
4. Documentation chapter in docs/book
5. Tests

---

## 13. Key Design Decisions

| Decision | Choice | Rationale |
| -------- | ------ | --------- |
| Page rendering | `stack()` page stack | Pages persist in stack, not rebuilt on navigate. Enables entry/exit animations. Suspended pages preserve scroll position, form state, etc. |
| Suspension | Freeze animations + buffer signals | Pages below top pay zero CPU. Leverage existing `motion_is_suspended` flag. Signals flush on resume = single batch rebuild. |
| Entry/exit animations | Per-page `AnimatedValue` springs on transition wrapper | Each page owns its own enter/exit progress. Spring physics = interruptible (can push during exit). `motion_on_ready_callback` for layout-aware start. |
| History storage | `Store<RouterHistory>` | Subscriber pattern = reactive updates for free. Thread-safe. Already proven in framework. |
| Route matching | Trie (built once) | O(path_depth) matching vs O(n) linear scan. Routes are static config. |
| FSM for transitions | `RouterState` + `PageState` impl `StateTransitions` | Prevents illegal state combos (e.g., two transitions at once). Event-driven = natural fit. PageState FSM per page in stack. |
| Deep link trait location | `blinc_platform` | Platform impls already exist there. Router doesn't need platform dependency. |
| Guard execution | Synchronous, sequential | Simple mental model. Async guards add complexity (loading states, cancellation). Can add async later. |
| Path syntax | Express-style (`:param`, `*wildcard`) | Industry standard. Familiar to web developers. Easy to parse. |

---

## 14. Files Modified in Other Crates

| File | Change |
| ---- | ------ |
| `Cargo.toml` (workspace) | Add `"crates/blinc_router"` to members |
| `blinc_platform/src/lib.rs` | Add `pub mod deep_link;` |
| `blinc_platform/src/deep_link.rs` | New file: `DeepLinkHandler` trait |
| `blinc_platform_desktop/src/lib.rs` | Implement `DeepLinkHandler` |
| `blinc_platform_android/src/lib.rs` | Implement `DeepLinkHandler` (JNI bridge) |
| `blinc_platform_ios/src/lib.rs` | Implement `DeepLinkHandler` (objc bridge) |
| `blinc_app/src/windowed.rs` | Wire deep link handler + router initialization |
| `blinc_app/src/android.rs` | Wire deep link handler |
| `blinc_app/src/ios.rs` | Wire deep link handler |
| `blinc_app/Cargo.toml` | Add `blinc_router` dependency |
