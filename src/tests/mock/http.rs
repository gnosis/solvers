use {
    anyhow::anyhow,
    std::{
        collections::HashSet,
        fmt::{self, Debug, Formatter},
        net::SocketAddr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
            Mutex,
        },
    },
    tokio::task::JoinHandle,
};

#[derive(Clone)]
pub enum Path {
    Any,
    Exact(String),
    Glob(glob::Pattern),
}

impl Path {
    pub fn exact(s: impl ToString) -> Self {
        Self::Exact(s.to_string())
    }

    pub fn glob(s: impl AsRef<str>) -> Self {
        Self::Glob(glob::Pattern::new(s.as_ref()).unwrap())
    }
}

impl PartialEq<Path> for String {
    fn eq(&self, path: &Path) -> bool {
        match path {
            Path::Any => true,
            Path::Exact(exact) => exact == self,
            Path::Glob(glob) => glob.matches(self),
        }
    }
}

impl Debug for Path {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Path::Any => f.debug_tuple("Any").finish(),
            Path::Exact(exact) => f
                .debug_tuple("Exact")
                .field(&format_args!("{exact}"))
                .finish(),
            Path::Glob(glob) => f
                .debug_tuple("Glob")
                .field(&format_args!("{}", glob.as_str()))
                .finish(),
        }
    }
}

pub fn abort_on_panic() {
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
}

#[derive(Clone, Debug)]
pub enum Expectation {
    Get {
        path: Path,
        res: serde_json::Value,
    },
    Post {
        path: Path,
        req: RequestBody,
        res: serde_json::Value,
    },
}

#[derive(Clone, Debug)]
pub enum RequestBody {
    /// The received `[RequestBody]` has to match the provided value exactly.
    Exact(serde_json::Value),
    /// The received `[RequestBody]` has to match the provided value partially
    /// excluding the specified paths which are represented as dot-separated
    /// strings.
    Partial(serde_json::Value, Vec<&'static str>),
    /// Any `[RequestBody]` will be accepted.
    Any,
}

/// Drop handle that will verify that the server task didn't panic throughout
/// the test and that all the expectations have been met.
pub struct ServerHandle {
    /// The address that handles requests to this server.
    pub address: SocketAddr,
    /// Handle to shut down the server task on drop.
    handle: JoinHandle<()>,
    /// Expectations that are left over after the test.
    expectations: Arc<Mutex<Vec<Expectation>>>,
    /// Indicates if some assertion failed.
    assert_failed: Arc<AtomicBool>,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Don't cause mass hysteria!
        if std::thread::panicking() {
            return;
        }

        let server_panicked = self.assert_failed.load(std::sync::atomic::Ordering::SeqCst);
        // Panics happening in the server task might not cause the test to fail and only
        // show up if some assertion fails in the main task. This accomplishes that.
        assert!(!server_panicked);

        assert!(
            !self.handle.is_finished(),
            "mock http server terminated before test ended"
        );
        assert_eq!(
            self.expectations.lock().unwrap().len(),
            0,
            "mock server did not receive enough requests"
        );
        self.handle.abort();
    }
}

/// Set up an mock external HTTP API.
pub async fn setup(mut expectations: Vec<Expectation>) -> ServerHandle {
    // Reverse expectations so test can specify them in natural order while allowing
    // us to simply `.pop()` the last element.
    expectations.reverse();

    let expectations = Arc::new(Mutex::new(expectations));
    let failed_assert = Arc::new(AtomicBool::new(false));

    let app = axum::Router::new()
        .route(
            "/*path",
            axum::routing::get(
                |axum::extract::State(state),
                 axum::extract::Path(path),
                 axum::extract::RawQuery(query)| async move {
                    axum::response::Json(get(state, Some(path), query))
                },
            )
                .post(
                    |axum::extract::State(state),
                     axum::extract::Path(path),
                     axum::extract::RawQuery(query),
                     axum::extract::Json(req)| async move {
                        axum::response::Json(post(state, Some(path), query, req))
                    },
                ),
        )
        // Annoying, but `axum` doesn't seem to match `/` with the above route,
        // so explicitly mount `/`.
        .route(
            "/",
            axum::routing::get(
                |axum::extract::State(state), axum::extract::RawQuery(query)| async move {
                    axum::response::Json(get(state, None, query))
                },
            )
                .post(
                    |axum::extract::State(state),
                     axum::extract::RawQuery(query),
                     axum::extract::Json(req)| async move {
                        axum::response::Json(post(state, None, query, req))
                    },
                ),
        )
        .with_state(State {
            expectations: expectations.clone(),
            failed_assert: failed_assert.clone(),
        });

    let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
    let address = server.local_addr();
    let handle = tokio::spawn(async move { server.await.unwrap() });

    ServerHandle {
        handle,
        expectations,
        address,
        assert_failed: failed_assert,
    }
}

#[derive(Clone)]
struct State {
    /// Endpoint handler reads from here which request to expect and what to
    /// respond.
    expectations: Arc<Mutex<Vec<Expectation>>>,
    /// Request handler notifies test about failed assert via this mutex.
    failed_assert: Arc<AtomicBool>,
}

/// Runs the given closure and updates a flag if it panics.
fn assert_and_propagate_panics<F, R>(assertions: F, flag: &AtomicBool) -> R
where
    F: FnOnce() -> R + std::panic::UnwindSafe + 'static,
{
    std::panic::catch_unwind(assertions)
        .map_err(|_| {
            flag.store(true, Ordering::SeqCst);
        })
        .expect("ignore this panic; it was caused by the previous panic")
}

fn get(state: State, path: Option<String>, query: Option<String>) -> serde_json::Value {
    let expectation = state.expectations.lock().unwrap().pop();
    let assertions = || {
        let (expected_path, res) = match expectation {
            Some(Expectation::Get { path, res }) => (path, res),
            Some(other) => panic!("expected GET request but got {other:?}"),
            None => panic!("got another GET request, but didn't expect any more"),
        };

        let full_path = full_path(path, query);
        assert_eq!(full_path, expected_path, "GET request has unexpected path");
        res
    };
    assert_and_propagate_panics(assertions, &state.failed_assert)
}

/// Asserts that two JSON values are equal, excluding specified paths.
///
/// This macro is used to compare two JSON values for equality while ignoring
/// certain paths in the JSON structure. The paths to be ignored are specified
/// as a list of dot-separated strings. If the two JSON values are not equal
/// (excluding the ignored paths), the macro will panic with a detailed error
/// message indicating the location of the discrepancy.
///
/// # Arguments
///
/// * `$actual` - The actual JSON value obtained in a test.
/// * `$expected` - The expected JSON value for comparison.
/// * `$exclude_paths` - An array of dot-separated strings specifying the paths
///   to be ignored during comparison.
///
/// # Panics
///
/// The macro panics if the actual and expected JSON values are not equal,
/// excluding the ignored paths.
///
/// # Examples
///
/// ```
/// let actual = json!({"user": {"id": 1, "name": "Alice", "email": "alice@example.com"}});
/// let expected = json!({"user": {"id": 1, "name": "Alice", "email": "bob@example.com"}});
/// assert_json_matches!(actual, expected, ["user.email"]);
/// ```
macro_rules! assert_json_matches {
    ($actual:expr, $expected:expr, $exclude_paths:expr) => {{
        let exclude_paths = parse_field_paths(&$exclude_paths);
        json_matches_excluding(&$actual, &$expected, &exclude_paths)
            .expect("JSON did not match with the exclusion of specified paths");
    }};
}

fn post(
    state: State,
    path: Option<String>,
    query: Option<String>,
    req: serde_json::Value,
) -> serde_json::Value {
    let expectation = state.expectations.lock().unwrap().pop();

    let assertions = move || {
        let (expected_path, expected_req, res) = match expectation {
            Some(Expectation::Post { path, req, res }) => (path, req, res),
            Some(other) => panic!("expected POST request but got {other:?}"),
            None => panic!("got another POST request, but didn't expect any more"),
        };

        let full_path = full_path(path, query);
        assert_eq!(full_path, expected_path, "POST request has unexpected path");
        match expected_req {
            RequestBody::Exact(value) => assert_eq!(req, value, "POST request has unexpected body"),
            RequestBody::Partial(value, exclude_paths) => {
                let exclude_paths = exclude_paths
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<&str>>();
                assert_json_matches!(req, value, &exclude_paths)
            }
            RequestBody::Any => (),
        }
        res
    };

    assert_and_propagate_panics(assertions, &state.failed_assert)
}

fn full_path(path: Option<String>, query: Option<String>) -> String {
    let path = path.unwrap_or_default();
    match query {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}

/// Parses dot-separated field paths into a set of paths.
fn parse_field_paths(paths: &[&str]) -> HashSet<Vec<String>> {
    paths
        .iter()
        .map(|path| path.split('.').map(String::from).collect())
        .collect()
}

/// Recursively compares two JSON values, excluding specified paths, and returns
/// detailed errors using anyhow.
fn json_matches_excluding(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    exclude_paths: &HashSet<Vec<String>>,
) -> anyhow::Result<()> {
    /// A helper function that recursively compares two JSON values using
    /// Depth-First Search (DFS) traversal. It utilizes a `current_path`
    /// accumulator to maintain the current path within the JSON structure,
    /// which is then compared against the `exclude_paths` parameter.
    /// During the backtracking process, the function updates the `current_path`
    /// accumulator to reflect the current position in the JSON structure.
    fn compare_jsons(
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        exclude_paths: &HashSet<Vec<String>>,
        current_path: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        match (actual, expected) {
            (serde_json::Value::Object(map_a), serde_json::Value::Object(map_b)) => {
                let keys: HashSet<_> = map_a.keys().chain(map_b.keys()).cloned().collect();
                for key in keys {
                    current_path.push(key.clone());

                    if exclude_paths.contains(current_path) {
                        current_path.pop();
                        continue;
                    }

                    match (map_a.get(&key), map_b.get(&key)) {
                        (Some(value_a), Some(value_b)) => {
                            if let Err(e) =
                                compare_jsons(value_a, value_b, exclude_paths, current_path)
                            {
                                current_path.pop();
                                return Err(e);
                            }
                        }
                        (None, Some(_)) => {
                            let error_msg = format!(
                                "Key missing in actual JSON at {}",
                                current_path.join("."),
                            );
                            current_path.pop();
                            return Err(anyhow!(error_msg));
                        }
                        (Some(_), None) => {
                            let error_msg = format!(
                                "Key missing in expected JSON at {}",
                                current_path.join("."),
                            );
                            current_path.pop();
                            return Err(anyhow!(error_msg));
                        }
                        (None, None) => unreachable!(),
                    }

                    current_path.pop();
                }
                Ok(())
            }
            _ => {
                if actual != expected {
                    Err(anyhow!(
                        "Mismatch at {}: {:?} != {:?}",
                        current_path.join("."),
                        actual,
                        expected
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    let mut current_path = vec![];
    compare_jsons(actual, expected, exclude_paths, &mut current_path)
}

#[cfg(test)]
mod tests {
    use {super::*, maplit::hashset, serde_json::json};

    #[test]
    fn test_parse_field_paths() {
        let paths = ["user.profile.name", "user.settings"];
        let parsed_paths = parse_field_paths(&paths);
        let expected_paths: HashSet<Vec<String>> = hashset! {
            vec!["user".to_string(), "profile".to_string(), "name".to_string()],
            vec!["user".to_string(), "settings".to_string()],
        };
        assert_eq!(parsed_paths, expected_paths)
    }

    #[test]
    fn test_json_matches_excluding_no_exclusions() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice"
                }
            }
        });
        assert_json_matches!(json_a, json_b, [])
    }

    #[test]
    fn test_json_matches_excluding_with_exclusions() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                },
                "enabled": true,
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2022-01-01T12:00:00Z"
                },
                "enabled": false,
            }
        });
        assert_json_matches!(json_a, json_b, ["user.profile.timestamp", "user.enabled"])
    }

    #[test]
    #[should_panic(
        expected = "JSON did not match with the exclusion of specified paths: Mismatch at \
                    user.profile.name: String(\"Alice\") != String(\"Bob\")"
    )]
    fn test_json_matches_excluding_failure() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Bob",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b, [])
    }

    #[test]
    #[should_panic(
        expected = "JSON did not match with the exclusion of specified paths: Key missing in \
                    expected JSON at user.profile.name"
    )]
    fn test_json_matches_excluding_key_is_missing() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b, [])
    }

    #[test]
    #[should_panic(
        expected = "JSON did not match with the exclusion of specified paths: Key missing in \
                    actual JSON at user.profile.name"
    )]
    fn test_json_matches_excluding_key_is_missing_reversed() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b, [])
    }
}
