use moostache::{HashMapLoader, TemplateLoader};
use maplit::hashmap;
use std::error::Error;
use indoc::indoc;
use serde_json::json;

// this examples shows:
// 1. how to initialize a HashMapLoader with
//    with a hashmap
// 2. how to render a template by passing it
//    a serde_json::Value

// run this example with:
// cargo run --example hashmaploader_json

fn main() -> Result<(), Box<dyn Error>> {
    let loader = HashMapLoader::try_from(hashmap! {
        "blog" => indoc! {"
            <h1>{{title}}</h1>
            {{#posts}}{{>post}}{{/posts}}
            {{^posts}}<span>No posts ;(</span>{{/posts}}
        "},
        "post" => indoc! {"
            <h2>{{title}}</h2>
            <span>{{teaser}}</span>
        "},
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