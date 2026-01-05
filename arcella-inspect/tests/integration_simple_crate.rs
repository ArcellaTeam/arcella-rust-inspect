use arcella_inspect::{analyze_project, analysis_to_yaml};
use std::path::Path;
use serde_yaml_ng as serde_yaml;

#[test]
fn test_simple_lib() {
    let root = Path::new("tests-data/simple");
    let analysis = analyze_project(root).unwrap();
    let yaml = analysis_to_yaml(&analysis, root).unwrap();

    // Check structure via deserialization
    let output: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    let functions = output["subprojects"][0]["functions"].as_sequence().unwrap();
    assert_eq!(functions.len(), 1);

    let func = &functions[0];
    assert_eq!(func["name"], "add");
    assert_eq!(func["parameters"].as_sequence().unwrap().len(), 2);
    assert_eq!(func["docstring"], "Adds two numbers.");
}