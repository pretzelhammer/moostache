use moostache::{FileLoader, LoaderConfig, TemplateLoader};
use std::error::Error;
use serde_derive::Serialize;
use indoc::indoc;

// this examples shows:
// 1. how to initialize a FileLoader with
//    with a LoaderConfig
// 2. how to render a template by passing it
//    a Rust type which impls Serialize

// run this example with:
// cargo run --example hashmaploader_config

#[derive(Serialize)]
struct Blog<'a> {
    title: &'a str,
    posts: Vec<Post<'a>>,
}

#[derive(Serialize)]
struct Post<'a> {
    title: &'a str,
    teaser: &'a str,
}

#[cfg(windows)]
const TEMPLATES_DIRECTORY: &str = ".\\examples\\templates\\";

#[cfg(not(windows))]
const TEMPLATES_DIRECTORY: &str = "./examples/templates/";

fn main() -> Result<(), Box<dyn Error>> {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_directory: TEMPLATES_DIRECTORY,
        templates_extension: "html",
        cache_size: 200,
    })?;

    let blog = Blog {
        title: "John's blog",
        posts: vec![
            Post {
                title: "Sitting on a bench",
                teaser: "Just sitting on a bench, eating chips..."
            },
            Post {
                title: "Walking down the street",
                teaser: "Just walking down the street, looking at birds..."
            },
        ]
    };

    // internally the Rust type is serialized into
    // a serde_json::Value using serde_json::to_value
    let rendered = loader.render_serializable_to_string("blog", &blog)?;
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