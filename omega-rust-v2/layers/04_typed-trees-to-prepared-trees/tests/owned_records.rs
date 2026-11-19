use omega_preparation::{PreparedTrees, PreparedValue, prepare_source_package};
use omega_resolver::{DeclarationId, ParsedSource, StageSource};
use omega_typer::{TypeError, resolve_source_package};

fn prepare(source: &str) -> Result<PreparedTrees<'_>, TypeError> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    prepare_source_package(&package)
}

fn declaration<'a>(
    prepared: &'a PreparedTrees<'_>,
    path: &str,
) -> &'a DeclarationId {
    prepared
        .typed()
        .resolved()
        .declarations
        .iter()
        .find(|id| id.package == "app" && id.path == path)
        .unwrap_or_else(|| panic!("missing app declaration {path}"))
}

#[test]
fn prepared_consumer_reads_declaration_order_and_the_exact_case() {
    let source = "data Choice [copy] { case Selected(left:u8, right:bool); case Empty; }
                  data Entry [copy] { count:u8; choice:Choice; }
                  const ROWS:[Entry;2]=[FIRST,SECOND];
                  const FIRST:Entry=Entry{choice: Choice::Selected{right:true, left:7}, count:3};
                  const SECOND:Entry=Entry{choice: Choice::Empty, count:4};";
    let prepared = prepare(source).unwrap();
    let entry = declaration(&prepared, "Entry");
    let choice = declaration(&prepared, "Choice");
    let selected = declaration(&prepared, "Choice::Selected");
    let empty = declaration(&prepared, "Choice::Empty");
    let first = PreparedValue::Record {
        owner: entry.clone(),
        fields: [
            PreparedValue::Integer(3.into()),
            PreparedValue::Sum {
                owner: choice.clone(),
                case: selected.clone(),
                fields: [PreparedValue::Integer(7.into()), PreparedValue::Bool(true)].into(),
            },
        ]
        .into(),
    };
    let second = PreparedValue::Record {
        owner: entry.clone(),
        fields: [
            PreparedValue::Integer(4.into()),
            PreparedValue::Sum {
                owner: choice.clone(),
                case: empty.clone(),
                fields: [].into(),
            },
        ]
        .into(),
    };
    assert_eq!(
        prepared.constant(declaration(&prepared, "ROWS")),
        Some(&PreparedValue::Array([first, second].into())),
    );
}

#[test]
fn a_case_with_the_same_name_in_another_sum_is_refused() {
    let source = "data First [copy] {case Empty;}
                  data Second [copy] {case Empty;}
                  const VALUE:First=Second::Empty;";
    let error = prepare(source).unwrap_err().message;
    assert!(error.contains("First") && error.contains("Second"), "{error}");
}

#[test]
fn applied_fields_use_the_selected_type_arguments() {
    let source = "data Cell<T [copy]> [copy] { item:T; }
                  data Maybe<T [copy]> [copy] { case Item(value:T); case Empty; }
                  const VALUE:Maybe<Cell<Cell<u8>>> = Maybe::<Cell<Cell<u8>>>::Item{
                      value:Cell::<Cell<u8>>{item:Cell::<u8>{item:5}}
                  };";
    let prepared = prepare(source).unwrap();
    let cell = declaration(&prepared, "Cell");
    let expected = PreparedValue::Sum {
        owner: declaration(&prepared, "Maybe").clone(),
        case: declaration(&prepared, "Maybe::Item").clone(),
        fields: [PreparedValue::Record {
            owner: cell.clone(),
            fields: [PreparedValue::Record {
                owner: cell.clone(),
                fields: [PreparedValue::Integer(5.into())].into(),
            }]
            .into(),
        }]
        .into(),
    };
    assert_eq!(prepared.constant(declaration(&prepared, "VALUE")), Some(&expected));
}

#[test]
fn field_initializers_evaluate_in_authored_order() {
    let source = "data Pair [copy] {first:u8; second:u8;}
                  machine add(a:u8,b:u8)->u8 {a+b}
                  machine mul(a:u8,b:u8)->u8 {a*b}
                  const VALUE:Pair=Pair{second:mul(200,2),first:add(255,1)};";
    let error = prepare(source).unwrap_err().message;
    assert!(error.contains("exact multiplication exceeds"), "{error}");
}

#[test]
fn a_bare_case_cannot_skip_its_carrier_predicate() {
    let source = "data Choice [copy] requires false {case Empty;}
                  const VALUE:Choice=Choice::Empty;";
    let error = prepare(source).unwrap_err().message;
    assert!(error.contains("selected constructor requires proof"), "{error}");
}

#[test]
fn freezing_checks_the_type_of_inactive_payloads() {
    let linear = "data Handle [linear] {}
                  data Choice [linear] {case Empty; case Used(handle:Handle);}
                  const VALUE:Choice=Choice::Empty;";
    let error = prepare(linear).unwrap_err().message;
    assert!(error.contains("complete constant type must permit copying"), "{error}");

    let cleanup = "data Handle [copy] dropped by dispose {}
                   machine dispose(handle:Handle) {}
                   data Choice<T [copy]> [copy] {case Empty; case Used(handle:T);}
                   const VALUE:Choice<Handle> = Choice::Empty;";
    let error = prepare(cleanup).unwrap_err().message;
    assert!(error.contains("complete constant type has a cleanup obligation"), "{error}");

    let loan = "data Choice [copy] {case Empty; case Used(view:@u8);}
                const VALUE:Choice=Choice::Empty;";
    let error = prepare(loan).unwrap_err().message;
    assert!(error.contains("complete constant type is outside the prepared value fragment"), "{error}");
}

#[test]
fn exact_byte_literals_freeze_as_owned_array_elements() {
    let source = r#"data Key [copy] { bytes:[u8;4]; }
                    const VALUE:Key=Key{bytes:("é\0\xFF")};
                    const EMPTY:[u8;0]="";
                    machine runtime()->@[u8] {"é\0\xFF"}"#;
    let prepared = prepare(source).unwrap();
    let bytes = [0xc3u8, 0xa9, 0, 0xff].map(|byte| PreparedValue::Integer(byte.into()));
    let key = PreparedValue::Record {
        owner: declaration(&prepared, "Key").clone(),
        fields: [PreparedValue::Array(bytes.into())].into(),
    };
    assert_eq!(prepared.constant(declaration(&prepared, "VALUE")), Some(&key));
    assert_eq!(prepared.constant(declaration(&prepared, "EMPTY")), Some(&PreparedValue::Array([].into())),);
}

#[test]
fn differently_sized_arrays_refuse_the_same_byte_literal() {
    for width in [1, 3] {
        let source = format!(r#"const VALUE:[u8;{width}]="é";"#);
        let error = prepare(&source).unwrap_err().message;
        assert_eq!(error, format!("byte literal has 2 bytes, but its array requires {width}"));
    }
}

#[test]
fn a_machine_returns_an_exact_quoted_byte_array() {
    let source = r#"machine key()->[u8;2]{("é")}
                    machine branch(b:bool)->[u8;2]{match b {true->("é"),false->"ab",}}
                    machine view()->@[u8]{"é"}
                    const DIRECT:[u8;2]=key();
                    const SELECTED:[u8;2]=branch(true);"#;
    let prepared = prepare(source).unwrap();
    let expected = PreparedValue::Array(
        [0xc3u8, 0xa9]
            .map(|byte| PreparedValue::Integer(byte.into()))
            .into(),
    );
    assert_eq!(prepared.constant(declaration(&prepared, "DIRECT")), Some(&expected));
    assert_eq!(prepared.constant(declaration(&prepared, "SELECTED")), Some(&expected));
    let short_result = source.replace("key()->[u8;2]", "key()->[u8;1]");
    let error = prepare(&short_result).unwrap_err();
    assert_eq!(error.span.slice(short_result.as_bytes()), Some("\"é\"".as_bytes()));
    assert_eq!(error.message, "byte literal has 2 bytes, but its array requires 1");
}

#[test]
fn a_machine_constructs_owned_values_for_closed_type_identity() {
    let source = "data Key [copy]{count:u8; active:bool;}
                  data Choice [copy]{case Item(key:Key); case Empty;}
                  machine key(count:u8)->Key requires count > 0 {Key{active:true,count:count}}
                  machine wrap(value:Key)->Choice {Choice::Item{key:value}}
                  machine array(value:Choice)->[Choice;1] {[value]}
                  machine identity(value:[Choice;1])->[Choice;1] {let saved:[Choice;1]=value; saved}
                  const GOT:[Choice;1]=identity(array(wrap(key(7))));
                  data Row<const V:[Choice;1]>{}
                  machine carry(x:Row<identity(array(wrap(key(7))))>)
                     ->Row<[Choice::Item{key:Key{count:7,active:true}}]>{x}";
    let prepared = prepare(source).unwrap();
    let carry = prepared
        .typed()
        .signatures
        .iter()
        .find(|s| s.machine.path == "carry")
        .unwrap();
    assert_eq!(carry.parameters[0].1, carry.result);
    let expected = PreparedValue::Array(
        [PreparedValue::Sum {
            owner: declaration(&prepared, "Choice").clone(),
            case: declaration(&prepared, "Choice::Item").clone(),
            fields: [PreparedValue::Record {
                owner: declaration(&prepared, "Key").clone(),
                fields: [PreparedValue::Integer(7.into()), PreparedValue::Bool(true)].into(),
            }]
            .into(),
        }]
        .into(),
    );
    assert_eq!(prepared.constant(declaration(&prepared, "GOT")), Some(&expected));
    let changed = source.replace("active:true,count:count", "active:false,count:count");
    let error = prepare(&changed).unwrap_err();
    assert_eq!(error.span.slice(changed.as_bytes()), Some(b"x".as_slice()));
    assert!(error.message.contains("false") && error.message.contains("true"), "{}", error.message);
    let rejected = source.replace("key(7)", "key(0)");
    let error = prepare(&rejected).unwrap_err();
    assert!(
        error.message.contains("precondition is false") && error.message.contains("key"),
        "{}",
        error.message
    );
}
