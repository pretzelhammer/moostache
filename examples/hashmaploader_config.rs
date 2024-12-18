use moostache::{HashMapLoader, TemplateLoader, LoaderConfig};
use std::error::Error;
use serde_json::json;
use indoc::indoc;

// this examples shows:
// 1. how to initialize a HashMapLoader with
//    with a LoaderConfig
// 2. how to render a template by passing it
//    a serde_json::Value

// run this example with:
// cargo run --example hashmaploader_config

fn main() -> Result<(), Box<dyn Error>> {
    let loader = HashMapLoader::try_from(LoaderConfig {
        templates_directory: "./templates/examples/",
        templates_extension: ".html",
        cache_size: 200,
    })?;
    let blog = json!({
        "title": "John's blog",
        "posts": [
            {
                "title": "Sitting on a bench",
                "teaser": "Just sitting on a bench, eating chips..."
            },
            {
                "title": "Walking down the street",
                "teaser": "Just walking down the street, looking at birds..."
            }
        ]
    });
    let rendered = loader.render_to_string("blog", &blog)?;
    let expected = indoc! {"
        <h1>John&#x27;s blog</h1>
        <h2>Sitting on a bench</h2>
        <span>Just sitting on a bench, eating chips...</span>
        <h2>Walking down the street</h2>
        <span>Just walking down the street, looking at birds...</span>
    "};
    assert_eq!(rendered.trim(), expected.trim());
    Ok(())
}