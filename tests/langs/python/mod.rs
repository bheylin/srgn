use rstest::rstest;
use srgn::scoping::{
    langs::python::{PremadePythonQuery, Python, PythonQuery},
    ScopedViewBuildStep,
};

use super::get_input_output;

#[rstest]
#[case("docstring.py", PythonQuery::Premade(PremadePythonQuery::DocStrings))]
#[case("comments.py", PythonQuery::Premade(PremadePythonQuery::Comments))]
#[case(
    "function-names.py",
    PythonQuery::Premade(PremadePythonQuery::FunctionNames)
)]
#[case(
    "function-calls.py",
    PythonQuery::Premade(PremadePythonQuery::FunctionCalls)
)]
fn test_python(#[case] file: &str, #[case] query: PythonQuery) {
    let lang = Python::new(query);

    let (input, output) = get_input_output("python", file);

    let mut view = lang.scope(&input).build();
    view.delete();

    assert_eq!(view.to_string(), output);
}