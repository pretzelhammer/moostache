<!-- moostache readme rendered on crates.io -->

moostache is a [Mustache](https://mustache.github.io/mustache.5.html) template engine.

It supports the following Mustache features: escaped variables, unescaped variables, dotted names, implicit iterators, sections, inverted sections, comments, and partials.

It does not support these Mustache features: lambdas, dynamic names, blocks, parents, or set delimiters.

## Guide

To render templates you must create a type that implements the `TemplateLoader` trait and call one of its render functions. Moostache provides two implementations: `HashMapLoader` and `FileLoader`.

### `HashMapLoader`

You can create a `HashMapLoader` from a hashmap:

```rust,ignore
use moostache::HashMapLoader;
use maplit::hashmap;

let loader = HashMapLoader::try_from(hashmap! {
    "greet" => "hello {{name}}!",
})?;
```

Or from a `LoaderConfig`:

```rust,ignore
use moostache::{HashMapLoader, LoaderConfig};

// this will eagerly load all .html files in the
// templates directory and its sub-directories
// into memory and compile them into moostache
// templates, up to cache_size
let loader = HashMapLoader::try_from(LoaderConfig {
    templates_directory: "./templates/",
    templates_extension: "html",
    cache_size: 200,
})?;
```

Then you can render any template by name, passing it a type which impls `Serialize`:

```rust,ignore
use moostache::TemplateLoader;
use serde_derive::Serialize;

#[derive(Serialize)]
struct Person<'a> {
    name: &'a str,
}

let john = Person {
    name: "John",
};

let rendered = loader.render_serializable_to_string("greet", &john);
assert_eq!(rendered, "hello John!")
```

Or by passing it a `serde_json::Value`:

```rust,ignore
use moostache::TemplateLoader;
use serde_json::json;

let john = json!({
    "name": "John",
});

let rendered = loader.render_to_string("greet", &john);
assert_eq!(rendered, "hello John!")
```

### `FileLoader`

You can create a `FileLoader` from a `LoaderConfig`:

```rust,ignore
use moostache::{FileLoader, LoaderConfig};

// this loader will lazily read .html files from
// the templates directory and its sub-directories
// and compile them to moostache templates during
// renders, and it will cache up to cache_size
// compiled templates in an internal LRU cache
let loader = FileLoader::try_from(LoaderConfig {
    templates_directory: "./templates/",
    templates_extension: "html",
    cache_size: 200,
})?;
```

Then, as explained above, you can render any template by name, passing it a type which impls `Serialize` or a `serde_json::Value`.

### `HashMapLoader` vs `FileLoader`

`HashMapLoader` is a little bit faster during initial renders because it requires you to preload all templates that may be used during the render into memory. You may prefer to use the `HashMapLoader` if all of your templates can fit into memory.

`FileLoader` is more memory-efficient, since it lazily fetches templates during renders, and then caches a certain amount of them in an LRU cache for follow-up renders. You may prefer to use the `FileLoader` if not all of your templates can fit into memory.

Regardless, both impl the `TemplateLoader` trait so they each support the `insert` and `remove` methods to insert and remove templates in-between renders for additional flexibility.
