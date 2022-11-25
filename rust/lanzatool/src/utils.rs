use std::path::Path;

// All Linux file paths should be convertable to strings
pub fn path_to_string(path: impl AsRef<Path>) -> String {
    String::from(path.as_ref().to_str().expect(&format!(
        "Failed to convert path '{}' to a string",
        path.as_ref().display()
    )))
}
