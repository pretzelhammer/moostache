use maplit::hashmap;

use super::*;

fn test_temp1(source: &'static str, frags: Vec<Fragment<'static>>) -> Template {
    Template {
        fragments: frags,
        skips: Vec::new(),
        source: source.into(),
    }
}

fn test_temp2(source: &'static str, frags: Vec<Fragment<'static>>, skips: Vec<SectionSkip>) -> Template {
    Template {
        fragments: frags,
        skips,
        source: source.into(),
    }
}

////////////////////////////////////
// TEST PARSING INVALID TEMPLATES //
////////////////////////////////////

#[test]
fn test_parse_empty() {
    let source = "";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorNoContent("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_escaped_variable() {
    let source = "{{ dfg%jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidEscapedVariableTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_escaped_variable() {
    let source = "{{ dfg.jgf }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidEscapedVariableTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_unescaped_variable() {
    let source = "{{{ dfg%jgf }}}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidUnescapedVariableTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_unescaped_variable() {
    let source = "{{{ dfg.jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidUnescapedVariableTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_comment() {
    let source = "{{! comment }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidCommentTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_section_start() {
    let source = "{{# dfg%jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionStartTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_section_start() {
    let source = "{{# dfg.jgf }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionStartTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_section_missing_end() {
    let source = "{{# dfg.jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorUnclosedSectionTags("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_section_mismatched_end() {
    let source = "{{# dfg.jgf }} lol {{/ not.same }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorMismatchedSectionEndTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_inverted_section_start() {
    let source = "{{^ dfg%jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidInvertedSectionStartTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_inverted_section_start() {
    let source = "{{^ dfg.jgf }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidInvertedSectionStartTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_inverted_section_missing_end() {
    let source = "{{^ dfg.jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorUnclosedSectionTags("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_inverted_section_mismatched_end() {
    let source = "{{^ dfg.jgf }} lol {{/ not.same }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorMismatchedSectionEndTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_section_end() {
    let source = "{{/ dfg%jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionEndTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_section_end() {
    let source = "{{/ dfg.jgf }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionEndTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_section_end_without_start() {
    let source = "{{/ dfg.jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorMismatchedSectionEndTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_invalid_partial() {
    let source = "{{> dfg\"jgf }}";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidPartialTag("".to_owned());
    assert_eq!(err, expected);
}

#[test]
fn test_parse_unclosed_partial() {
    let source = "{{> dfg/jgf }";
    let err = Template::parse(source).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidPartialTag("".to_owned());
    assert_eq!(err, expected);
}

//////////////////////////////////
// TEST PARSING VALID TEMPLATES //
//////////////////////////////////

#[test]
fn test_parse_literal() {
    let source = "hello world";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_template = test_temp1(
        source, 
        vec![Fragment::Literal(source)]
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_escaped_var() {
    let source = "{{name}}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source, 
        vec![Fragment::EscapedVariable("name")]
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_escaped_var_padded() {
    let source = "{{  name  }}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::EscapedVariable("name")]
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_unescaped_var() {
    let source = "{{{name}}}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::UnescapedVariable("name")]
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_unescaped_var_padded() {
    let source = "{{{  name  }}}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::UnescapedVariable("name")]
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_section() {
    let source = "{{# whatever }} cheese {{/ whatever}}";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_frags = vec![
        Fragment::Section("whatever"),
        Fragment::Literal(" cheese "),
    ];
    let expected_skips = vec![SectionSkip {
        nested_sections: 0,
        nested_fragments: 1,
    }];
    let expected_template = test_temp2(
        source,
        expected_frags,
        expected_skips,
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_partial() {
    let source = "{{>name}}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::Partial("name")],
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_partial_padded() {
    let source = "{{>  name  }}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::Partial("name")],
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_partial_nested_padded() {
    let source = "{{>  name/in/nested/dir  }}";
    let template = Template::parse(source)
        .expect("Fragment parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![Fragment::Partial("name/in/nested/dir".into())],
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_v1_features() {
    let source = "prefix {{ escaped }}!";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![
            Fragment::Literal("prefix ".into()),
            Fragment::EscapedVariable("escaped".into()),
            Fragment::Literal("!".into()),
        ],
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_v2_features() {
    let source = "{{! comment }}prefix {{ escaped }} {{{ unescaped }}}!";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_template = test_temp1(
        source,
        vec![
            Fragment::Literal("prefix ".into()),
            Fragment::EscapedVariable("escaped".into()),
            Fragment::Literal(" ".into()),
            Fragment::UnescapedVariable("unescaped".into()),
            Fragment::Literal("!".into()),
        ],
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_v3_features() {
    let source = "{{! comment }}prefix {{ escaped }} {{{ unescaped }}} {{# section }} {{ cheese }} {{/ section }}!";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_frags = vec![
        Fragment::Literal("prefix "),
        Fragment::EscapedVariable("escaped"),
        Fragment::Literal(" "),
        Fragment::UnescapedVariable("unescaped"),
        Fragment::Literal(" "),
        Fragment::Section("section"),
        Fragment::Literal(" "),
        Fragment::EscapedVariable("cheese"),
        Fragment::Literal(" "),
        Fragment::Literal("!"),
    ];
    let expected_skips = vec![SectionSkip {
        nested_fragments: 3,
        nested_sections: 0,
    }];
    let expected_template = test_temp2(
        source,
        expected_frags,
        expected_skips,
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_v4_features() {
    let source = "{{> nested/partial }}{{! comment }}prefix {{ escaped }} {{{ unescaped }}} {{# section }} {{ cheese }} {{/ section }}{{^section}}no cheese damn{{/section}}!";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_frags = vec![
        Fragment::Partial("nested/partial"),
        Fragment::Literal("prefix "),
        Fragment::EscapedVariable("escaped"),
        Fragment::Literal(" "),
        Fragment::UnescapedVariable("unescaped"),
        Fragment::Literal(" "),
        Fragment::Section("section"),
        Fragment::Literal(" "),
        Fragment::EscapedVariable("cheese"),
        Fragment::Literal(" "),
        Fragment::InvertedSection("section"),
        Fragment::Literal("no cheese damn"),
        Fragment::Literal("!"),
    ];
    let expected_skips = vec![
        SectionSkip {
            nested_fragments: 3,
            nested_sections: 0,
        },
        SectionSkip {
            nested_fragments: 1,
            nested_sections: 0,
        },
    ];
    let expected_template = test_temp2(
        source,
        expected_frags,
        expected_skips,
    );
    assert_eq!(template, expected_template);
}

#[test]
fn test_parse_heavy_section_nesting() {
    let source = "prefix{{#s1}}infix1{{#s1a}}infix2{{#s1aa}}content-1aa{{/s1aa}}{{^s1aa}}nothing-1aa{{/s1aa}}{{#s1ab}}content-1ab{{/s1ab}}{{^s1ab}}nothing-1ab{{/s1ab}}{{/s1a}}{{^s1a}}nothing-1a{{/s1a}}infix3{{#s1b}}content-1b{{/s1b}}{{^s1b}}nothing-1b{{/s1b}}infix4{{/s1}}suffix";
    let template = Template::parse(source)
        .expect("template parsed successfully");
    let expected_frags = vec![
        Fragment::Literal("prefix"),
        Fragment::Section("s1"),
        Fragment::Literal("infix1"),
        Fragment::Section("s1a"),
        Fragment::Literal("infix2"),
        Fragment::Section("s1aa"),
        Fragment::Literal("content-1aa"),
        Fragment::InvertedSection("s1aa"),
        Fragment::Literal("nothing-1aa"),
        Fragment::Section("s1ab"),
        Fragment::Literal("content-1ab"),
        Fragment::InvertedSection("s1ab"),
        Fragment::Literal("nothing-1ab"),
        Fragment::InvertedSection("s1a"),
        Fragment::Literal("nothing-1a"),
        Fragment::Literal("infix3"),
        Fragment::Section("s1b"),
        Fragment::Literal("content-1b"),
        Fragment::InvertedSection("s1b"),
        Fragment::Literal("nothing-1b"),
        Fragment::Literal("infix4"),
        Fragment::Literal("suffix"),
    ];
    let expected_skips = vec![
        SectionSkip { // s1
            nested_sections: 8,
            nested_fragments: 19,
        },
        SectionSkip { // s1a
            nested_sections: 4,
            nested_fragments: 9,
        },
        SectionSkip { // s1aa
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // ^s1aa
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // s1ab
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // ^s1ab
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // ^s1a
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // s1b
            nested_sections: 0,
            nested_fragments: 1,
        },
        SectionSkip { // ^s1b
            nested_sections: 0,
            nested_fragments: 1,
        },
    ];
    let expected_template = test_temp2(
        source,
        expected_frags,
        expected_skips,
    );
    assert_eq!(template, expected_template);
}

/////////////////////////////
// TEST JSON TRUTHY VALUES //
/////////////////////////////

#[test]
fn test_truthy_value_null() {
    assert_eq!(is_truthy(&json!(null)), false);
}

#[test]
fn test_truthy_value_false() {
    assert_eq!(is_truthy(&json!(false)), false);
}

#[test]
fn test_truthy_value_true() {
    assert_eq!(is_truthy(&json!(true)), true);
}

#[test]
fn test_truthy_value_zero() {
    assert_eq!(is_truthy(&json!(0)), false);
}

#[test]
fn test_truthy_value_nonzero() {
    assert_eq!(is_truthy(&json!(1)), true);
}

#[test]
fn test_truthy_value_empty_string() {
    assert_eq!(is_truthy(&json!("")), false);
}

#[test]
fn test_truthy_value_nonempty_string() {
    assert_eq!(is_truthy(&json!("hello")), true);
}

#[test]
fn test_truthy_value_empty_array() {
    assert_eq!(is_truthy(&json!([])), false);
}

#[test]
fn test_truthy_value_nonempty_array() {
    assert_eq!(is_truthy(&json!([1])), true);
}

#[test]
fn test_truthy_value_empty_object() {
    assert_eq!(is_truthy(&json!({})), false);
}

#[test]
fn test_truthy_value_nonempty_object() {
    assert_eq!(is_truthy(&json!({"field": 1})), true);
}

////////////////////////////////
// TEST RESOLVING JSON VALUES //
////////////////////////////////

#[test]
fn test_resolve_value_dot() {
    assert_eq!(
        resolve_value(
            ".",
            &[&json!("hello")],
        ),
        &json!("hello"),
    );
}

#[test]
fn test_resolve_value_number() {
    assert_eq!(
        resolve_value(
            "0",
            &[&json!(["hello"])],
        ),
        &json!("hello"),
    );
}

#[test]
fn test_resolve_value_string() {
    assert_eq!(
        resolve_value(
            "greeting",
            &[&json!({"greeting": "hello"})],
        ),
        &json!("hello"),
    );
}

#[test]
fn test_resolve_value_nested_numbers() {
    assert_eq!(
        resolve_value(
            "1.1",
            &[&json!([1, [2, 3], 4])],
        ),
        &json!(3),
    );
}

#[test]
fn test_resolve_value_nested_strings() {
    assert_eq!(
        resolve_value(
            "a.b",
            &[&json!({"a": {"b": 1}})],
        ),
        &json!(1),
    );
}

#[test]
fn test_resolve_value_nested_mixed() {
    assert_eq!(
        resolve_value(
            "1.a.0.b",
            &[&json!([0, {"a": [{"b": 1}]}])],
        ),
        &json!(1),
    );
}

#[test]
fn test_resolve_value_parent_scope_number() {
    assert_eq!(
        resolve_value(
            "0",
            &[&json!([2]), &json!({"a": 1})],
        ),
        &json!(2),
    );

    // array indexes should not fall thru
    assert_eq!(
        resolve_value(
            "2",
            &[&json!([0, 1, 2]), &json!([0])],
        ),
        &json!(null),
    );
}

#[test]
fn test_resolve_value_parent_scope_string() {
    assert_eq!(
        resolve_value(
            "a",
            &[&json!({"a": 1}), &json!([2])],
        ),
        &json!(1),
    );
}

#[test]
fn test_write_value_null() {
    let mut writer = Vec::new();
    let _ = write_value(&json!(null), &mut writer);
    assert!(writer.is_empty());
}

///////////////////////////////////////////////
// TEST RENDERING TEMPLATES WITHOUT PARTIALS //
///////////////////////////////////////////////

#[test]
fn test_render_section_object_parent_scope() {
    let source = "{{ blogTitle }}, posts: {{# posts }}{{ postTitle }} by {{ author }}, {{/ posts}}";
    let data = json!({
        "blogTitle": "blog title",
        "author": "chris",
        "posts": [
            {
                "postTitle": "post 1"
            },
            {
                "postTitle": "post 2"
            }
        ]
    });
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "blog title, posts: post 1 by chris, post 2 by chris, ";
    assert_eq!(rendered, expected);
}


#[test]
fn test_render_section_array_multi_escaped() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!(["&", "<", ">", "\"", "'"]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "&amp;&lt;&gt;&quot;&#x27;";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_array_multi_unescaped() {
    let source = "{{# . }}{{{ . }}}{{/ . }}";
    let data = json!(["&", "<", ">", "\"", "'"]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "&<>\"'";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_object() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!({ "some": "field" });
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "{&quot;some&quot;:&quot;field&quot;}";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_array_single() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!([1]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "1";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_string_escaped() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!("&<>\"'");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "&amp;&lt;&gt;&quot;&#x27;";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_string_unescaped() {
    let source = "{{# . }}{{{ . }}}{{/ . }}";
    let data = json!("&<>\"'");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "&<>\"'";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_float() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!(0.1);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "0.1";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_integer() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!(1);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "1";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_true() {
    let source = "{{# . }}{{ . }}{{/ . }}";
    let data = json!(true);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "true";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_empty_object() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!({});
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_empty_array() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!([]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_empty_string() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!("");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_zero() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!(0);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_false() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!(false);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_inverted_section_null() {
    let source = "{{^ . }}lol{{/ . }}";
    let data = json!(null);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "lol";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_empty_object() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!({});
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_empty_array() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!([]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_empty_string() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!("");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_zero() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!(0);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_false() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!(false);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_section_null() {
    let source = "{{# . }}lol{{/ . }}";
    let data = json!(null);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_nested_key_string() {
    let source = "hello {{ name.last }}!";
    let data = json!({
        "name": {
            "first": "mister",
            "last": "&<>\"'"
        }
    });
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello &amp;&lt;&gt;&quot;&#x27;!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_nested_index_string() {
    let source = "hello {{ 1.1 }}!";
    let data = json!(["john", ["moon", "&<>\"'"], "chris"]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello &amp;&lt;&gt;&quot;&#x27;!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_nested_key_string() {
    let source = "hello {{{ name.last }}}!";
    let data = json!({
        "name": {
            "first": "mister",
            "last": "world"
        }
    });
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_nested_index_string() {
    let source = "hello {{{ 1.1 }}}!";
    let data = json!(["john", ["moon", "world"], "chris"]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_key_string() {
    let source = "hello {{{ name }}}!";
    let data = json!({"name": "world"});
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_index_string() {
    let source = "hello {{{ 1 }}}!";
    let data = json!(["john", "world", "chris"]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_object() {
    let source = "hello {{ . }}!";
    let data = json!({"some": "field"});
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello {&quot;some&quot;:&quot;field&quot;}!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_array() {
    let source = "hello {{ . }}!";
    let data = json!([1, "string", null]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello [1,&quot;string&quot;,null]!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_null() {
    let source = "hello {{ . }}!";
    let data = json!(null);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello !";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_float() {
    let source = "hello {{ . }}!";
    let data = json!(123.5);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello 123.5!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_integer() {
    let source = "hello {{ . }}!";
    let data = json!(123);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello 123!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_escaped_dot_string() {
    let source = "hello {{ . }}!";
    let data = json!("&<>\"'");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello &amp;&lt;&gt;&quot;&#x27;!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_object() {
    let source = "hello {{{ . }}}!";
    let data = json!({"some": "field"});
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello {\"some\":\"field\"}!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_array() {
    let source = "hello {{{ . }}}!";
    let data = json!([1, "string", null]);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello [1,\"string\",null]!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_null() {
    let source = "hello {{{ . }}}!";
    let data = json!(null);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello !";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_float() {
    let source = "hello {{{ . }}}!";
    let data = json!(123.5);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello 123.5!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_integer() {
    let source = "hello {{{ . }}}!";
    let data = json!(123);
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello 123!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unescaped_dot_string() {
    let source = "hello {{{ . }}}!";
    let data = json!("world");
    let template = Template::parse(source).unwrap();
    let rendered = template.render_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_serializable_string() {
    let source = "hello {{{ . }}}!";
    let data = "world".to_owned();
    let template = Template::parse(source).unwrap();
    let rendered = template.render_serializable_no_partials_to_string(&data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_serializable_struct() {
    let source = "hello {{ name }}!";
    #[derive(serde_derive::Serialize)]
    struct Person {
        name: &'static str,
    }
    let data = Person {
        name: "homer"
    };
    let template = Template::parse(source).unwrap();
    let rendered = template.render_serializable_no_partials_to_string(&data).unwrap();
    let expected = "hello homer!";
    assert_eq!(rendered, expected);
}

////////////////////////////////////////////
// TEST RENDERING TEMPLATES WITH PARTIALS //
////////////////////////////////////////////

#[test]
fn test_render_partial_hashmap() {
    let source = "{{>partial}}!";
    let data = json!(null);
    let mut loader = HashMapLoader::try_from(hashmap! {
        "partial" => "hello world",
    }).unwrap();
    let template = Template::parse(source).unwrap();
    let rendered = template.render_to_string(&mut loader, &data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_padded_hashmap() {
    let source = "{{>  partial  }}!";
    let data = json!(null);
    let mut loader = HashMapLoader::try_from(hashmap! {
        "partial" => "hello world",
    }).unwrap();
    let template = Template::parse(source).unwrap();
    let rendered = template.render_to_string(&mut loader, &data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_hashmap_from_config() {
    let source = "{{>greet}}!";
    let data = json!({"name": "world"});
    let loader = HashMapLoader::try_from(
        LoaderConfig::default()
    ).unwrap();
    let template = Template::parse(source).unwrap();
    let rendered = dbg!(template.render_to_string(&loader, &data)).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_hashmap_from_config_too_many_templates() {
    let err = HashMapLoader::try_from(LoaderConfig {
        cache_size: 1,
        ..LoaderConfig::default()
    }).unwrap_err();
    let expected = MoostacheError::ConfigErrorTooManyTemplates;
    assert_eq!(err, expected);
}

#[test]
fn test_render_partial_file() {
    let source = "{{>greet}}!";
    let data = json!({"name": "world"});
    let loader = FileLoader::try_from(
        LoaderConfig::default()
    ).unwrap();
    let template = Template::parse(source).unwrap();
    let rendered = template.render_to_string(&loader, &data).unwrap();
    let expected = "hello world!";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partials_exceed_cache() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".tpl",
        cache_size: 1, // cache size of only 1
        ..LoaderConfig::default()
    }).unwrap();
    // now render a partial chain of 3 nested templates
    let rendered = loader.render_to_string(
        "nesting-0",
        &json!({"message": "hello world"})
    ).unwrap();
    let expected = "level 0 level 1 level 2 hello world";
    assert_eq!(rendered, expected);
}

////////////////////////////////////////////////////////
// TEST RENDERING TEMPLATES WITH PARTIALS WITH ERRORS //
////////////////////////////////////////////////////////

#[test]
fn test_render_partial_invalid_comment() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-comment", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidCommentTag("error/invalid-comment".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_escaped_variable() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-escaped-variable", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidEscapedVariableTag("error/invalid-escaped-variable".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_unescaped_variable() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-unescaped-variable", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidUnescapedVariableTag("error/invalid-unescaped-variable".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_inverted_section_start() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-inverted-section-start", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidInvertedSectionStartTag("error/invalid-inverted-section-start".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_section_start() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-section-start", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionStartTag("error/invalid-section-start".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_section_end() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-section-end", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidSectionEndTag("error/invalid-section-end".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_mismatched_section_end() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/mismatched-section-end", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorMismatchedSectionEndTag("error/mismatched-section-end".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_unclosed_sections() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/unclosed-sections", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorUnclosedSectionTags("error/unclosed-sections".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_invalid_partial() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/invalid-partial", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorInvalidPartialTag("error/invalid-partial".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_no_content() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/no-content", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::ParseErrorNoContent("error/no-content".into());
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_partial_nonexistent_partial() {
    let loader = FileLoader::try_from(LoaderConfig {
        templates_extension: ".error",
        ..LoaderConfig::default()
    }).unwrap();
    let rendered = loader.render_to_string(
        "calls-error/nonexistent-partial", 
        &json!(null),
    ).unwrap_err();
    let expected = MoostacheError::IoError(
        "doesnt/exist".into(),
        std::io::ErrorKind::NotFound,
    );
    assert_eq!(rendered, expected);
}

//////////////////////////////////////
// TEST MOOSTACHEERROR DISPLAY IMPL //
//////////////////////////////////////

#[test]
fn test_moostache_error_display_impl() {
    use MoostacheError::*;
    let mut err = ParseErrorGeneric("".into());
    assert_eq!("error parsing anonymous template", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template", &err.to_string());

    err = ParseErrorNoContent("".into());
    assert_eq!("error parsing anonymous template: empty template", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: empty template", &err.to_string());

    err = ParseErrorUnclosedSectionTags("".into());
    assert_eq!("error parsing anonymous template: unclosed section tags", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: unclosed section tags", &err.to_string());

    err = ParseErrorInvalidEscapedVariableTag("".into());
    assert_eq!("error parsing anonymous template: invalid escaped variable tag, expected {{ variable }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid escaped variable tag, expected {{ variable }}", &err.to_string());

    err = ParseErrorInvalidUnescapedVariableTag("".into());
    assert_eq!("error parsing anonymous template: invalid unescaped variable tag, expected {{{ variable }}}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid unescaped variable tag, expected {{{ variable }}}", &err.to_string());

    err = ParseErrorInvalidSectionEndTag("".into());
    assert_eq!("error parsing anonymous template: invalid section eng tag, expected {{/ section }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid section eng tag, expected {{/ section }}", &err.to_string());

    err = ParseErrorMismatchedSectionEndTag("".into());
    assert_eq!("error parsing anonymous template: mismatched section eng tag, expected {{# section }} ... {{/ section }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: mismatched section eng tag, expected {{# section }} ... {{/ section }}", &err.to_string());

    err = ParseErrorInvalidCommentTag("".into());
    assert_eq!("error parsing anonymous template: invalid comment tag, expected {{! comment }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid comment tag, expected {{! comment }}", &err.to_string());

    err = ParseErrorInvalidSectionStartTag("".into());
    assert_eq!("error parsing anonymous template: invalid section start tag, expected {{# section }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid section start tag, expected {{# section }}", &err.to_string());

    err = ParseErrorInvalidInvertedSectionStartTag("".into());
    assert_eq!("error parsing anonymous template: invalid inverted section start tag, expected {{^ section }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid inverted section start tag, expected {{^ section }}", &err.to_string());

    err = ParseErrorInvalidPartialTag("".into());
    assert_eq!("error parsing anonymous template: invalid partial tag, expected {{> partial }}", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error parsing \"name\" template: invalid partial tag, expected {{> partial }}", &err.to_string());

    err = IoError("".into(), std::io::ErrorKind::NotFound);
    assert_eq!("error reading anonymous template: entity not found", &err.to_string());
    err = err.set_name("name");
    assert_eq!("error reading \"name\" template: entity not found", &err.to_string());

    err = LoaderErrorTemplateNotFound("".into());
    assert_eq!("loader error: anonymous template not found", &err.to_string());
    err = err.set_name("name");
    assert_eq!("loader error: \"name\" template not found", &err.to_string());

    err = LoaderErrorNonUtf8FilePath("some.file".into());
    assert_eq!("loader error: can't load non-utf8 file path: some.file", &err.to_string());

    err = ConfigErrorNonPositiveCacheSize;
    assert_eq!("config error: cache size must be positive", &err.to_string());

    err = ConfigErrorInvalidTemplatesDirectory("some.file".into());
    assert_eq!("config error: invalid templates directory: some.file", &err.to_string());

    err = ConfigErrorTooManyTemplates;
    assert_eq!("config error: templates in directory exceeds cache size", &err.to_string());

    err = SerializationError;
    assert_eq!("serialization error: could not serialize data to serde_json::Value", &err.to_string());
}
