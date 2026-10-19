use fluidic_ui_idl::{Source, check};

fn source(text: &str) -> Vec<Source> {
    vec![Source::new("test.idl", text)]
}

fn errors(text: &str) -> Vec<String> {
    match check(&source(text)) {
        Ok(_) => panic!("expected the schema to be refused"),
        Err(diagnostics) => diagnostics.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn accepts_a_minimal_schema() {
    let schema = check(&source(
        "contract 0.1\n\
         struct sdk_info since 0.1 {\n\
             name: string since 0.1\n\
         }\n",
    ))
    .expect("valid schema");

    assert_eq!(schema.contract.to_string(), "0.1");
    let names: Vec<&str> = schema.types.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["sdk_info"]);
}

#[test]
fn a_syntax_error_names_its_file_line_and_column() {
    assert_eq!(
        errors("contract 0.1\nstruct a since 0.1 {\n    x string since 0.1\n}\n"),
        ["test.idl:3:7: expected `:`, found `string`"]
    );
}

#[test]
fn the_contract_is_declared_exactly_once() {
    assert_eq!(
        errors("struct a since 0.1 {}\n"),
        ["test.idl:1:1: the schema has no `contract` declaration"]
    );
    assert_eq!(
        errors("contract 0.1\ncontract 0.2\n"),
        ["test.idl:2:1: `contract` is declared again; it was declared at test.idl:1:1"]
    );
}

#[test]
fn names_are_snake_case_and_not_reserved() {
    assert_eq!(
        errors("contract 0.1\nstruct SdkInfo since 0.1 {\n    app__id: string since 0.1\n}\n"),
        [
            "test.idl:2:8: `SdkInfo` is not a snake_case name",
            "test.idl:3:5: `app__id` is not a snake_case name",
        ]
    );
    assert_eq!(
        errors(
            "contract 0.1\n\
             enum e since 0.1 {\n    unknown since 0.1\n    string since 0.1\n}\n\
             struct string since 0.1 {\n    version: version since 0.1\n    unknown: u32 since 0.1\n}\n"
        ),
        [
            "test.idl:3:5: `unknown` is reserved",
            "test.idl:6:8: `string` is reserved",
            "test.idl:8:5: `unknown` is reserved",
        ]
    );
}

#[test]
fn names_are_unique_in_their_scope() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             struct a since 0.1 {\n    x: u32 since 0.1\n    x: u32 since 0.1\n}\n\
             enum a since 0.1 { x since 0.1 }\n"
        ),
        [
            "test.idl:4:5: `x` is declared again; it was declared at test.idl:3:5",
            "test.idl:6:6: `a` is declared again; it was declared at test.idl:2:8",
        ]
    );
}

#[test]
fn every_named_type_is_declared() {
    assert_eq!(
        errors("contract 0.1\nstruct a since 0.1 {\n    x: list<b> since 0.1\n}\n"),
        ["test.idl:3:5: field `x` names `b`, which is not declared"]
    );
}

#[test]
fn a_variant_carries_a_struct_only() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             enum e since 0.1 { x since 0.1 }\n\
             union u since 0.1 {\n    v(e) since 0.1\n    w(nothing) since 0.1\n}\n"
        ),
        [
            "test.idl:4:5: variant `v` carries `e`, which is an enum, not a struct",
            "test.idl:5:5: variant `w` carries `nothing`, which is not declared",
        ]
    );
}

#[test]
fn a_member_is_never_older_than_its_parent_nor_newer_than_the_contract() {
    assert_eq!(
        errors(
            "contract 0.2\n\
             union u since 0.2 {\n    v since 0.1\n}\n\
             enum e since 0.3 { x since 0.3 }\n"
        ),
        [
            "test.idl:3:5: `v` is since 0.1, earlier than `u` (since 0.2)",
            "test.idl:5:6: `e` is since 0.3, later than the contract (0.2)",
            "test.idl:5:20: `x` is since 0.3, later than the contract (0.2)",
        ]
    );
}

#[test]
fn a_field_added_after_its_struct_is_optional() {
    assert_eq!(
        errors(
            "contract 0.2\n\
             message m from app since 0.1 {\n    a: u32 since 0.1\n    b: u32 since 0.2\n    optional c: u32 since 0.2\n}\n\
             union u since 0.1 {\n    v {\n        x: u32 since 0.2\n    } since 0.2\n}\n"
        ),
        ["test.idl:4:5: field `b` is added after `m` (since 0.1), so it must be optional"]
    );
}

#[test]
fn a_type_contains_itself_only_through_a_list_or_map() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             struct node since 0.1 {\n    children: list<node> since 0.1\n    optional next: link since 0.1\n}\n\
             union link since 0.1 {\n    to(node) since 0.1\n}\n"
        ),
        ["test.idl:2:8: `node` contains itself through `link` without a list or map between"]
    );
}

#[test]
fn generated_names_do_not_collide() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             union bye_reason since 0.1 {\n    done since 0.1\n}\n\
             struct bye_reason_done since 0.1 {}\n\
             struct app_message since 0.1 {}\n"
        ),
        [
            "test.idl:5:8: `bye_reason_done` becomes the Go name `ByeReasonDone`, which `done` at test.idl:3:5 also becomes",
            "test.idl:6:8: `app_message` becomes the name `AppMessage`, which the generator uses for the messages an app sends",
        ]
    );
}

#[test]
fn a_union_carries_each_struct_once() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             struct s since 0.1 {}\n\
             union u since 0.1 {\n    a(s) since 0.1\n    b(s) since 0.1\n}\n"
        ),
        ["test.idl:5:5: variant `b` carries `s`, which `a` at test.idl:4:5 already carries"]
    );
}

#[test]
fn a_type_does_not_take_a_name_from_the_rust_prelude() {
    assert_eq!(
        errors("contract 0.1\nstruct option since 0.1 {}\n"),
        [
            "test.idl:2:8: `option` becomes the name `Option`, which the generator uses for Rust's own `Option`"
        ]
    );
}

#[test]
fn member_names_do_not_collide_in_rust_or_go() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             struct s since 0.1 {\n    id: u32 since 0.1\n    i_d: u32 since 0.1\n}\n\
             struct t since 0.1 {}\n\
             union u since 0.1 {\n    x1(s) since 0.1\n    x_1(t) since 0.1\n}\n\
             enum e since 0.1 {\n    a1 since 0.1\n    a_1 since 0.1\n}\n"
        ),
        [
            "test.idl:4:5: `i_d` becomes the Go name `ID`, which `id` at test.idl:3:5 also becomes",
            "test.idl:9:5: `x_1` becomes the Rust name `X1`, which `x1` at test.idl:8:5 also becomes",
            "test.idl:13:5: `a_1` becomes the Go name `EA1`, which `a1` at test.idl:12:5 also becomes",
            "test.idl:13:5: `a_1` becomes the Rust name `A1`, which `a1` at test.idl:12:5 also becomes",
        ]
    );
}

#[test]
fn a_type_does_not_take_a_generated_function_name() {
    assert_eq!(
        errors("contract 0.1\nstruct item since 0.1 {}\nstruct encode_item since 0.1 {}\n"),
        [
            "test.idl:3:8: `encode_item` becomes the Go name `EncodeItem`, which `item` at test.idl:2:8 also becomes"
        ]
    );
}

#[test]
fn a_message_is_not_named_unknown() {
    assert_eq!(
        errors("contract 0.1\nmessage unknown from app since 0.1 {}\n"),
        [
            "test.idl:2:9: a message cannot be named `unknown`: the message unions have an unknown case"
        ]
    );
}

#[test]
fn an_enum_or_union_has_at_least_one_case() {
    assert_eq!(
        errors("contract 0.1\nenum e since 0.1 {}\nunion u since 0.1 {}\n"),
        [
            "test.idl:2:6: enum `e` has no values",
            "test.idl:3:7: union `u` has no variants",
        ]
    );
}

#[test]
fn a_variant_does_not_take_a_generated_name() {
    assert_eq!(
        errors("contract 0.1\nunion app since 0.1 {\n    message_unknown since 0.1\n}\n"),
        [
            "test.idl:3:5: `message_unknown` becomes the name `AppMessageUnknown`, which the generator uses for the messages an app sends"
        ]
    );
}

#[test]
fn a_type_does_not_take_a_codec_function_name() {
    assert_eq!(
        errors("contract 0.1\nstruct u_32 since 0.1 {}\n"),
        ["test.idl:2:8: `u_32` becomes the name `readU32`, which the generator uses for its codec"]
    );
    assert_eq!(
        errors("contract 0.1\nstruct float since 0.1 {}\n"),
        [
            "test.idl:2:8: `float` becomes the name `writeFloat`, which the generator uses for its codec"
        ]
    );
}

#[test]
fn self_is_reserved_where_rust_would_spell_it_self() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             struct self since 0.1 {\n    self: u32 since 0.1\n}\n\
             enum target since 0.1 {\n    self since 0.1\n}\n"
        ),
        [
            "test.idl:2:8: `self` is reserved",
            "test.idl:6:5: `self` is reserved",
        ]
    );
}

#[test]
fn a_keyword_is_free_as_a_variant_but_not_to_clash() {
    assert_eq!(
        errors(
            "contract 0.1\n\
             union app since 0.1 {\n    message since 0.1\n}\n\
             enum host since 0.1 {\n    message since 0.1\n}\n"
        ),
        [
            "test.idl:3:5: `message` becomes the name `AppMessage`, which the generator uses for the messages an app sends",
            "test.idl:6:5: `message` becomes the name `HostMessage`, which the generator uses for the messages a host sends",
        ]
    );
}

#[test]
fn a_case_does_not_take_a_types_encode_or_decode_function() {
    assert_eq!(
        errors(
            "contract 0.1\nstruct foo since 0.1 {}\nenum encode since 0.1 {\n    foo since 0.1\n}\n"
        ),
        [
            "test.idl:4:5: `foo` becomes the Go name `EncodeFoo`, which `foo` at test.idl:2:8 also becomes"
        ]
    );
}

#[test]
fn a_doc_comment_is_one_plain_paragraph() {
    let block =
        |name: &str| format!("a doc comment is one plain paragraph, and this line starts {name}");
    for (doc, problem) in [
        (
            "a\u{d}b",
            "a doc comment cannot hold control characters".to_string(),
        ),
        (
            "a\u{202e}b",
            "a doc comment cannot hold bidirectional formatting characters".to_string(),
        ),
        (
            "see[^1] now",
            "a doc comment cannot hold a footnote".to_string(),
        ),
        ("  indented", block("an indented block")),
        ("\tindented", block("an indented block")),
        ("> quote", block("a quote")),
        ("# heading", block("a heading")),
        ("- item", block("a list")),
        ("* item", block("a list")),
        ("+ item", block("a list")),
        ("1. item", block("a list")),
        ("2) item", block("a list")),
        ("```", block("a code block")),
        ("~~~rust", block("a code block")),
        ("[x]: https://example.com", block("a link definition")),
        ("| a | b |", block("a table")),
        ("<div>", block("an HTML block")),
        (
            "---",
            block("a rule, an underline or a table's delimiter row"),
        ),
        (
            "==",
            block("a rule, an underline or a table's delimiter row"),
        ),
        (
            "=",
            block("a rule, an underline or a table's delimiter row"),
        ),
        (
            "_ _ _",
            block("a rule, an underline or a table's delimiter row"),
        ),
        (
            "--|--",
            block("a rule, an underline or a table's delimiter row"),
        ),
        (
            ":-:|:-:",
            block("a rule, an underline or a table's delimiter row"),
        ),
    ] {
        let space = if doc.starts_with([' ', '\t']) {
            ""
        } else {
            " "
        };
        assert_eq!(
            errors(&format!(
                "contract 0.1\n///{space}{doc}\nstruct a since 0.1 {{}}\n"
            )),
            [format!("test.idl:2:1: {problem}")],
            "for {doc:?}"
        );
    }
}

#[test]
fn a_doc_comment_has_no_empty_line_inside() {
    assert_eq!(
        errors("contract 0.1\n///\n/// Intro.\n///\n/// Overview\n///\nstruct a since 0.1 {}\n"),
        ["test.idl:4:1: a doc comment is one paragraph, without empty lines"]
    );
}

#[test]
fn prose_may_hold_the_characters_that_start_blocks() {
    for doc in [
        "a - b, x > y and 1.5 are prose",
        "*emphasis* first",
        "#hashtag first",
        "[a link](https://example.com) first",
        "`code` first, then a | pipe",
        "2026 was a year",
        "a | b is prose on its own",
        "x - y = z, and :-) too",
    ] {
        let text = format!("contract 0.1\n/// {doc}\nstruct a since 0.1 {{}}\n");
        assert!(check(&source(&text)).is_ok(), "{doc:?} was refused");
    }
}
