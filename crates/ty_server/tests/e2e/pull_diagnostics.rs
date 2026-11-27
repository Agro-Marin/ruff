use std::time::Duration;

use anyhow::Result;
use insta::{assert_compact_json_snapshot, assert_debug_snapshot};
use lsp_server::RequestId;
use lsp_types::{
    DocumentDiagnosticReport, FileChangeType, PartialResultParams, PreviousResultId,
    ProgressNotification, Uri, WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressParams,
    WorkspaceDiagnosticParams, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportPartialResult,
    WorkspaceDocumentDiagnosticReport,
};
use lsp_types::{TextDocumentContentChangeWholeDocument, WorkspaceDiagnosticRequest};
use ruff_db::system::SystemPath;
use ty_server::{ClientOptions, DiagnosticMode};

use crate::diagnostic_snapshots::{
    condensed_document_diagnostic_snapshot, condensed_workspace_diagnostic_snapshot,
};
use crate::{AwaitResponseError, TestServer, TestServerBuilder};

#[test]
fn on_did_open() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo() -> str:
    return 42
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_debug_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn baselined_diagnostic_is_a_hint_and_refreshes() -> Result<()> {
    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let baseline_path = SystemPath::new("src/baselines/ty.json");
    let foo_content = "\
def foo() -> str:
    return 42
";
    let baseline = r#"{
        "version": 0,
        "files": {
            "foo.py": [{
                "rule": "invalid-return-type",
                "startColumn": 11,
                "endColumn": 13,
                "lineCount": 1
            }]
        }
    }"#;

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(
            "src/ty.toml",
            "\
baseline = 'baselines/ty.json'
",
        )?
        .with_file(baseline_path, baseline)?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);
    insta::assert_debug_snapshot!(condensed_document_diagnostic_snapshot(diagnostics), @r#""1:11..1:13[HINT]: Return type does not match returned value: expected `str`, found `Literal[42]`\n\ninfo: Suppressed by baseline `baselines/ty.json`""#);

    server.write_file(
        baseline_path,
        r#"{
            "version": 0,
            "files": {}
        }"#,
    )?;
    server.did_change_watched_file(baseline_path, FileChangeType::Changed);
    let diagnostics = server.document_diagnostic_request(foo, None);
    insta::assert_debug_snapshot!(condensed_document_diagnostic_snapshot(diagnostics), @r#""1:11..1:13[ERROR]: Return type does not match returned value: expected `str`, found `Literal[42]`""#);
    Ok(())
}

#[test]
fn unused_binding_has_unnecessary_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo():
    x = 1
    return 0
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn unreachable_code_has_unnecessary_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo():
    return 0
    print(\"dead\")
    print(\"still dead\")
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn current_analysis_unreachable_code_has_unnecessary_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let ty_toml = SystemPath::new("ty.toml");
    let foo_content = "\
import sys

if sys.version_info >= (3, 11):
    from typing import Self
";
    let ty_toml_content = "\
[environment]
python-version = \"3.10\"
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .with_file(ty_toml, ty_toml_content)?
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn unreachable_code_suppresses_unused_binding_hint() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo():
    return 0
    x = 1
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn workspace_reports_unused_binding_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo():
    x = 1
    return 0
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(
            workspace_root,
            Some(ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace)),
        )?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    let diagnostics = server.workspace_diagnostic_request(None, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn workspace_reports_unreachable_code_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo():
    return 0
    print(\"dead\")
    print(\"still dead\")
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(
            workspace_root,
            Some(ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace)),
        )?
        .with_file(foo, foo_content)?
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    let diagnostics = server.workspace_diagnostic_request(None, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn workspace_reports_current_analysis_unreachable_code_hint_tag() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let ty_toml = SystemPath::new("ty.toml");
    let foo_content = "\
import sys

if sys.version_info >= (3, 11):
    from typing import Self
";
    let ty_toml_content = "\
[environment]
python-version = \"3.10\"
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(
            workspace_root,
            Some(ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace)),
        )?
        .with_file(foo, foo_content)?
        .with_file(ty_toml, ty_toml_content)?
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    let diagnostics = server.workspace_diagnostic_request(None, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn loop_carried_rebinding_is_not_reported_unused() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def buy_sell_once(prices: list[float]) -> float:
    assert len(prices) > 1
    best_buy, best_so_far = prices[0], 0.0
    for i in range(1, len(prices)):
        best_so_far = max(best_so_far, prices[i] - best_buy)
        best_buy = min(best_buy, prices[i])
    return best_so_far
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics, @r#"{"items": [], "kind": "full"}"#);

    Ok(())
}

#[test]
fn on_did_open_diagnostics_off() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo() -> str:
    return 42
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(
            workspace_root,
            Some(ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Off)),
        )?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics, @r#"{"items": [], "kind": "full"}"#);

    Ok(())
}

#[test]
fn invalid_syntax_with_syntax_errors_disabled() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo(
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(
            workspace_root,
            Some(ClientOptions::default().with_show_syntax_errors(false)),
        )?
        .with_file(foo, foo_content)?
        .with_initialization_options(
            &ClientOptions::default()
                .with_show_syntax_errors(false)
                .with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);
    let diagnostics = server.document_diagnostic_request(foo, None);

    assert_compact_json_snapshot!(diagnostics, @r#"{"items": [], "kind": "full"}"#);

    let request_id = send_workspace_diagnostic_request(&mut server);
    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id);
    let workspace_diagnostics = shutdown_and_await_workspace_diagnostic(server, &request_id);
    assert_compact_json_snapshot!(workspace_diagnostics, @r#"{"items": []}"#);

    Ok(())
}

/// Regression test for <https://github.com/astral-sh/ty/issues/2310>
#[test]
fn stack_size() -> Result<()> {
    use std::fmt::Write;

    let _filter = filter_result_id();

    let mut content = String::new();
    writeln!(
        &mut content,
        "
from typing_extensions import reveal_type
total = 1{plus_one_repeated}
reveal_type(total)
        ",
        plus_one_repeated = " + 1".repeat(2000 - 1)
    )?;

    let file_path = SystemPath::new("src/foo.py");

    let mut server = TestServerBuilder::new()?
        .with_workspace(SystemPath::new("src"), None)?
        .with_file(file_path, &content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(file_path, content, 1);
    let diagnostics = server.document_diagnostic_request(file_path, None);

    assert_compact_json_snapshot!(diagnostics);

    Ok(())
}

#[test]
fn pull_excluded_file() -> Result<()> {
    let _filter = filter_result_id();

    let main_path = SystemPath::new("src/foo.py");
    let main_content = "reveal_type(\"included\")\n";

    let excluded_path = SystemPath::new("src/excluded/lib.py");
    let excluded_content = r#"reveal_type("Excluded")"#;

    let config = r#"
[src]
exclude = ["src/excluded/"]
"#;

    let mut server = TestServerBuilder::new()?
        .with_workspace(SystemPath::new("src"), None)?
        .with_file(main_path, main_content)?
        .with_file(excluded_path, excluded_content)?
        .with_file(SystemPath::new("ty.toml"), config)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(main_path, main_content, 1);
    let main_diagnostics = server.document_diagnostic_request(main_path, None);
    assert_compact_json_snapshot!("main", main_diagnostics);

    server.open_text_document(excluded_path, excluded_content, 1);
    let excluded_diagnostics = server.document_diagnostic_request(excluded_path, None);
    assert_compact_json_snapshot!("excluded", excluded_diagnostics);

    Ok(())
}

#[test]
fn document_diagnostic_caching_unchanged() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content = "\
def foo() -> str:
    return 42
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content, 1);

    let first_response = server.document_diagnostic_request(foo, None);

    let result_id = match &first_response {
        DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) => report
            .full_document_diagnostic_report
            .result_id
            .as_ref()
            .expect("First response should have a result ID")
            .clone(),
        DocumentDiagnosticReport::RelatedUnchangedDocumentDiagnosticReport(_) => {
            panic!("First response should be a full report")
        }
    };

    let second_response = server.document_diagnostic_request(foo, Some(result_id));

    match second_response {
        DocumentDiagnosticReport::RelatedUnchangedDocumentDiagnosticReport(_) => {
        }
        DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(_) => {
            panic!("Expected an unchanged report when diagnostics haven't changed")
        }
    }

    Ok(())
}

#[test]
fn document_diagnostic_caching_changed() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content_v1 = "\
def foo() -> str:
    return 42
";
    let foo_content_v2 = "\
def foo() -> str:
    return \"fixed\"
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content_v1)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content_v1, 1);

    let first_response = server.document_diagnostic_request(foo, None);

    let result_id = match &first_response {
        DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) => report
            .full_document_diagnostic_report
            .result_id
            .as_ref()
            .expect("First response should have a result ID")
            .clone(),
        DocumentDiagnosticReport::RelatedUnchangedDocumentDiagnosticReport(_) => {
            panic!("First response should be a full report")
        }
    };

    server.change_text_document(
        foo,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: foo_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    let second_response = server.document_diagnostic_request(foo, Some(result_id));

    match second_response {
        DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) => {
            assert_eq!(report.full_document_diagnostic_report.items.len(), 0);
        }
        DocumentDiagnosticReport::RelatedUnchangedDocumentDiagnosticReport(_) => {
            panic!("Expected a full report when diagnostics have changed")
        }
    }

    Ok(())
}

#[test]
fn document_diagnostic_caching_rendered_source_changed() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let foo = SystemPath::new("src/foo.py");
    let foo_content_v1 = "\
def foo() -> str:
    return 42  # before
";
    let foo_content_v2 = "\
def foo() -> str:
    return 42  # after!
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content_v1)?
        .with_full_diagnostic_output()
        .enable_pull_diagnostics(true)
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(foo, foo_content_v1, 1);

    let first_response = server.document_diagnostic_request(foo, None);
    let result_id = match &first_response {
        DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) => report
            .full_document_diagnostic_report
            .result_id
            .clone()
            .expect("First response should have a result ID"),
        DocumentDiagnosticReport::RelatedUnchangedDocumentDiagnosticReport(_) => {
            panic!("First response should be a full report")
        }
    };
    assert_debug_snapshot!(
        "document_diagnostic_caching_rendered_source_before",
        first_response
    );

    server.change_text_document(
        foo,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: foo_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    let second_response = server.document_diagnostic_request(foo, Some(result_id));
    assert_debug_snapshot!(
        "document_diagnostic_caching_rendered_source_after",
        second_response
    );

    Ok(())
}

/// Settings invalidate cached workspace results only when the reported diagnostics change.
#[test]
fn workspace_diagnostic_caching_settings_changed() -> Result<()> {
    let root = SystemPath::new("src");
    let extra = SystemPath::new("extra");
    let main = root.join("main.py");
    let unchanged = root.join("unchanged.py");
    let mut server = TestServerBuilder::new()?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .with_workspace(root, None)?
        .with_file(&main, "(")?
        .with_file(&unchanged, "missing")?
        .with_file(extra.join("empty.py"), "")?
        .build()
        .wait_until_workspaces_are_initialized();

    let first_response = server.workspace_diagnostic_request(None, None);
    let previous_result_ids = extract_result_ids_from_response(&first_response);

    server.add_workspace_folder(
        extra,
        Some(ClientOptions::default().with_show_syntax_errors(false)),
    )?;
    server.change_workspace_folders([extra], []);
    server = server.wait_until_workspaces_are_initialized();

    let mut response = server.workspace_diagnostic_request(None, Some(previous_result_ids));
    sort_workspace_diagnostic_response(&mut response);
    let [
        WorkspaceDocumentDiagnosticReport::WorkspaceFullDocumentDiagnosticReport(report),
        WorkspaceDocumentDiagnosticReport::WorkspaceUnchangedDocumentDiagnosticReport(
            unchanged_report,
        ),
    ] = response.items.as_slice()
    else {
        anyhow::bail!("Expected syntax errors to be cleared and other diagnostics to be unchanged");
    };
    assert_eq!(report.uri, server.file_uri(&main));
    assert!(report.full_document_diagnostic_report.items.is_empty());
    assert!(report.full_document_diagnostic_report.result_id.is_none());
    assert_eq!(unchanged_report.uri, server.file_uri(&unchanged));

    Ok(())
}

#[test]
fn workspace_diagnostic_caching() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");

    let file_a = SystemPath::new("src/unchanged.py");
    let file_a_content = "\
def foo() -> str:
    return 42  # This error will remain the same
";

    let file_b = SystemPath::new("src/new_error.py");
    let file_b_content_v1 = "\
def foo() -> int:
    return 42  # No error initially
";
    let file_b_content_v2 = "\
def foo() -> str:
    return 42  # Error appears
";

    let file_c = SystemPath::new("src/fixed_error.py");
    let file_c_content_v1 = "\
def foo() -> str:
    return 42  # Error initially
";
    let file_c_content_v2 = "\
def foo() -> str:
    return \"fixed\"  # Error removed
";

    let file_d = SystemPath::new("src/changed_error.py");
    let file_d_content_v1 = "\
def foo() -> str:
    return 42  # First error: expected str, got int
";
    let file_d_content_v2 = "\
def foo() -> int:
    return \"hello\"  # Different error: expected int, got str
";

    let file_e = SystemPath::new("src/modified_same_error.py");
    let file_e_content_v1 = "\
def foo() -> str:
    return 42  # Error: expected str, got int
";
    let file_e_content_v2 = "\
def bar() -> int:
    return 100  # New function added at the top

def foo() -> str:
    return 42  # Same error: expected str, got int
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .with_file(file_a, file_a_content)?
        .with_file(file_b, file_b_content_v1)?
        .with_file(file_c, file_c_content_v1)?
        .with_file(file_d, file_d_content_v1)?
        .with_file(file_e, file_e_content_v1)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(file_a, file_a_content, 1);

    let mut first_response = server.workspace_diagnostic_request(
        Some(lsp_types::ProgressToken::String("progress-1".to_string())),
        None,
    );
    sort_workspace_diagnostic_response(&mut first_response);

    assert_debug_snapshot!("workspace_diagnostic_initial_state", first_response);

    consume_all_progress_notifications(&mut server)?;

    let previous_result_ids = extract_result_ids_from_response(&first_response);

    server.open_text_document(file_b, file_b_content_v1, 1);
    server.open_text_document(file_c, file_c_content_v1, 1);
    server.open_text_document(file_d, file_d_content_v1, 1);
    server.open_text_document(file_e, file_e_content_v1, 1);

    server.change_text_document(
        file_b,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_b_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    server.change_text_document(
        file_c,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_c_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    server.change_text_document(
        file_d,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_d_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    server.change_text_document(
        file_e,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_e_content_v2.to_string(),
                },
            ),
        ],
        2,
    );

    let mut second_response = server.workspace_diagnostic_request(
        Some(lsp_types::ProgressToken::String("progress-2".to_string())),
        Some(previous_result_ids),
    );
    sort_workspace_diagnostic_response(&mut second_response);

    consume_all_progress_notifications(&mut server)?;

    assert_debug_snapshot!("workspace_diagnostic_after_changes", second_response);

    Ok(())
}

#[test]
#[cfg(unix)]
fn workspace_diagnostic_caching_unchanged_with_colon_in_path() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("astral:test");
    let foo = SystemPath::new("astral:test/test.py");
    let foo_content = "\
def foo() -> str:
    return 42
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(foo, foo_content)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .build()
        .wait_until_workspaces_are_initialized();

    let first_response = server.workspace_diagnostic_request(None, None);

    let mut previous_result_ids = extract_result_ids_from_response(&first_response);

    for previous_id in &mut previous_result_ids {
        previous_id
            .uri
            .set_path(&previous_id.uri.path().replace(':', "%3A"));
    }

    let workspace_request_id =
        server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        });

    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &workspace_request_id);

    let second_response = shutdown_and_await_workspace_diagnostic(server, &workspace_request_id);

    insta::assert_compact_debug_snapshot!(second_response, @"WorkspaceDiagnosticReport { items: [] }");

    Ok(())
}

pub(crate) fn filter_result_id() -> insta::internals::SettingsBindDropGuard {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r#""[a-f0-9]{16}""#, r#""[RESULT_ID]""#);
    settings.bind_to_scope()
}

fn consume_all_progress_notifications(server: &mut TestServer) -> Result<()> {
    let begin_params = server.await_notification::<lsp_types::ProgressNotification>();

    let Ok(_) = serde_json::from_value::<WorkDoneProgressBegin>(begin_params.value) else {
        return Err(anyhow::anyhow!("Expected Begin progress notification"));
    };

    loop {
        let params = server.await_notification::<lsp_types::ProgressNotification>();

        if serde_json::from_value::<WorkDoneProgressEnd>(params.value).is_ok() {
            break;
        }
    }

    Ok(())
}

/// Tests that the server sends partial results for workspace diagnostics
/// if a client sets the `partial_result_token` in the request.
///
/// Note: In production, the server throttles the partial results to one every 50ms. However,
/// this behavior makes testing very hard. That's why the server, in tests, sends a partial response
/// as soon as it batched at least 2 diagnostics together.
#[test]
fn workspace_diagnostic_streaming() -> Result<()> {
    const NUM_FILES: usize = 5;

    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");

    let error_content = "\
def foo() -> str:
    return 42  # Type error: expected str, got int
";

    let mut builder = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        );

    for i in 0..NUM_FILES {
        let file_path_string = format!("src/file_{i:03}.py");
        let file_path = SystemPath::new(&file_path_string);
        builder = builder.with_file(file_path, error_content)?;
    }

    let mut server = builder.build().wait_until_workspaces_are_initialized();

    let partial_token = lsp_types::ProgressToken::String("streaming-diagnostics".to_string());
    let request_id = server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: Some(partial_token.clone()),
        },
    });

    let mut received_results = 0usize;

    let final_response = server.await_response::<WorkspaceDiagnosticRequest>(&request_id);

    let response_items = final_response.items;

    assert_eq!(response_items.len(), 1);
    received_results += response_items.len();

    while let Ok(params) =
        server.try_await_notification::<ProgressNotification>(Some(Duration::from_secs(1)))
    {
        if params.token == partial_token {
            let report =
                serde_json::from_value::<WorkspaceDiagnosticReportPartialResult>(params.value)
                    .unwrap();
            let streamed_items = report.items;

            assert_eq!(streamed_items.len(), 2);
            received_results += streamed_items.len();

            if received_results == NUM_FILES {
                break;
            }
        }
    }

    assert_eq!(received_results, NUM_FILES);

    Ok(())
}

#[test]
fn workspace_pull_finishes_when_all_diagnostics_were_streamed() -> Result<()> {
    let mut server = TestServerBuilder::new()?
        .with_workspace(SystemPath::new("src"), None)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .with_file("src/a.py", "missing_a\n")?
        .with_file("src/b.py", "missing_b\n")?
        .build()
        .wait_until_workspaces_are_initialized();

    let token = lsp_types::ProgressToken::String("diagnostics".into());
    let request = server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams {
            partial_result_token: Some(token.clone()),
        },
    });

    let progress = server.await_notification::<ProgressNotification>();
    assert_eq!(progress.token, token);
    let batch: WorkspaceDiagnosticReportPartialResult = serde_json::from_value(progress.value)?;
    assert_eq!(batch.items.len(), 2);

    let response = server
        .try_await_response::<WorkspaceDiagnosticRequest>(&request, Some(Duration::from_secs(2)))
        .expect("a pull that streamed changed diagnostics must finish");
    assert!(response.items.is_empty());
    Ok(())
}

/// Tests that the server's diagnostic streaming (partial results) work correctly
/// with result ids.
#[test]
fn workspace_diagnostic_streaming_with_caching() -> Result<()> {
    const NUM_FILES: usize = 7;

    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let error_content = "def foo() -> str:\n    return 42  # Error";
    let changed_content = "def foo() -> str:\n    return true  # Error";

    let mut builder = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        );

    for i in 0..NUM_FILES {
        let file_path_string = format!("src/error_{i}.py");
        let file_path = SystemPath::new(&file_path_string);
        builder = builder.with_file(file_path, error_content)?;
    }

    let mut server = builder.build().wait_until_workspaces_are_initialized();

    server.open_text_document(SystemPath::new("src/error_0.py"), error_content, 1);
    server.open_text_document(SystemPath::new("src/error_1.py"), error_content, 1);
    server.open_text_document(SystemPath::new("src/error_2.py"), error_content, 1);

    let first_response = server.workspace_diagnostic_request(None, None);

    let result_ids = extract_result_ids_from_response(&first_response);

    assert_eq!(result_ids.len(), NUM_FILES);

    server.change_text_document(
        SystemPath::new("src/error_0.py"),
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: changed_content.to_string(),
                },
            ),
        ],
        2,
    );

    server.change_text_document(
        SystemPath::new("src/error_1.py"),
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: changed_content.to_string(),
                },
            ),
        ],
        2,
    );

    server.change_text_document(
        SystemPath::new("src/error_2.py"),
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: changed_content.to_string(),
                },
            ),
        ],
        2,
    );

    let partial_token = lsp_types::ProgressToken::String("streaming-diagnostics".to_string());
    let request2_id =
        server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: result_ids,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: Some(partial_token.clone()),
            },
        });

    let final_response2 = server.await_response::<WorkspaceDiagnosticRequest>(&request2_id);

    let mut all_items = Vec::new();

    let items = final_response2.items;

    assert_eq!(items.len(), NUM_FILES - 3 + 1);

    all_items.extend(items);

    while let Ok(params) =
        server.try_await_notification::<ProgressNotification>(Some(Duration::from_secs(1)))
    {
        if params.token == partial_token {
            let report =
                serde_json::from_value::<WorkspaceDiagnosticReportPartialResult>(params.value)
                    .unwrap();
            let streamed_items = report.items;

            assert_eq!(streamed_items.len(), 2);
            all_items.extend(streamed_items);

            if all_items.len() == NUM_FILES {
                break;
            }
        }
    }

    sort_workspace_report_items(&mut all_items);

    assert_debug_snapshot!(all_items);

    Ok(())
}

pub(crate) fn sort_workspace_diagnostic_response(response: &mut WorkspaceDiagnosticReport) {
    sort_workspace_report_items(&mut response.items);
}

fn sort_workspace_report_items(items: &mut [WorkspaceDocumentDiagnosticReport]) {
    fn item_uri(item: &WorkspaceDocumentDiagnosticReport) -> &Uri {
        match item {
            WorkspaceDocumentDiagnosticReport::WorkspaceFullDocumentDiagnosticReport(
                full_report,
            ) => &full_report.uri,
            WorkspaceDocumentDiagnosticReport::WorkspaceUnchangedDocumentDiagnosticReport(
                unchanged_report,
            ) => &unchanged_report.uri,
        }
    }

    items.sort_unstable_by(|a, b| item_uri(a).cmp(item_uri(b)));
}

/// The LSP specification requires that the server sends a response for every request.
///
/// This test verifies that the server responds to a long-polling workspace diagnostic request
/// when the server is shut down.
#[test]
fn workspace_diagnostic_long_polling_responds_on_shutdown() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let file_path = SystemPath::new("src/test.py");
    let file_content = "\
def hello() -> str:
    return \"world\"
";

    let mut server = create_workspace_server_with_file(workspace_root, file_path, file_content)?;

    let request_id = send_workspace_diagnostic_request(&mut server);

    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id);

    let workspace_response = shutdown_and_await_workspace_diagnostic(server, &request_id);

    assert_debug_snapshot!(
        "workspace_diagnostic_long_polling_shutdown_response",
        workspace_response
    );

    Ok(())
}

/// Tests that the server responds to a long-polling workspace diagnostic request
/// after a change introduced a new diagnostic.
#[test]
fn workspace_diagnostic_long_polling_responds_on_change() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let file_path = SystemPath::new("src/test.py");
    let file_content_no_error = "\
def hello() -> str:
    return \"world\"
";
    let file_content_with_error = "\
def hello() -> str:
    return 42  # Type error: expected str, got int
";

    let mut server =
        create_workspace_server_with_file(workspace_root, file_path, file_content_no_error)?;

    server.open_text_document(file_path, file_content_no_error, 1);

    let request_id = send_workspace_diagnostic_request(&mut server);

    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id);

    server.change_text_document(
        file_path,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_content_with_error.to_string(),
                },
            ),
        ],
        2,
    );

    let workspace_response = server.await_response::<WorkspaceDiagnosticRequest>(&request_id);

    assert_debug_snapshot!(
        "workspace_diagnostic_long_polling_change_response",
        workspace_response
    );

    Ok(())
}

/// The LSP specification requires that the server responds to each request with exactly one response.
///
/// This test verifies that the server sends one response (and not two) if a long polling workspace diagnostic request
/// is cancelled.
#[test]
fn workspace_diagnostic_long_polling_responds_on_cancellation() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let file_path = SystemPath::new("src/test.py");
    let file_content = "\
def hello() -> str:
    return \"world\"
";

    let mut server = create_workspace_server_with_file(workspace_root, file_path, file_content)?;

    let request_id = send_workspace_diagnostic_request(&mut server);

    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id);

    server.cancel(&request_id);

    let result = server.try_await_response::<WorkspaceDiagnosticRequest>(&request_id, None);
    assert_debug_snapshot!(
        "workspace_diagnostic_long_polling_cancellation_result",
        result
    );


    Ok(())
}

/// This test verifies an entire workspace diagnostic cycle with long-polling:
/// * Initial suspend with no diagnostics
/// * Change the file to introduce a diagnostic, server should respond with the new diagnostics
/// * Send a second workspace diagnostic request, which should suspend again because the diagnostics haven't changed
/// * Change the file again to fix the diagnostic, server should respond with no diagnostics
#[test]
fn workspace_diagnostic_long_polling_suspend_change_suspend_cycle() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let file_path = SystemPath::new("src/test.py");
    let file_content_no_error = "\
def hello() -> str:
    return \"world\"
";
    let file_content_with_error = "\
def hello() -> str:
    return 42  # Type error: expected str, got int
";
    let file_content_fixed = "\
def hello() -> str:
    return \"fixed\"
";

    let mut server =
        create_workspace_server_with_file(workspace_root, file_path, file_content_no_error)?;

    server.open_text_document(file_path, file_content_no_error, 1);

    let request_id_1 = send_workspace_diagnostic_request(&mut server);
    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id_1);

    server.change_text_document(
        file_path,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_content_with_error.to_string(),
                },
            ),
        ],
        2,
    );

    let first_response = server.await_response::<WorkspaceDiagnosticRequest>(&request_id_1);

    let previous_result_ids = extract_result_ids_from_response(&first_response);

    let request_id_2 =
        server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        });

    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id_2);

    server.change_text_document(
        file_path,
        vec![
            lsp_types::TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                TextDocumentContentChangeWholeDocument {
                    text: file_content_fixed.to_string(),
                },
            ),
        ],
        3,
    );

    let second_response = server.await_response::<WorkspaceDiagnosticRequest>(&request_id_2);

    assert_debug_snapshot!(
        "workspace_diagnostic_suspend_change_suspend_first_response",
        first_response
    );
    assert_debug_snapshot!(
        "workspace_diagnostic_suspend_change_suspend_second_response",
        second_response
    );

    Ok(())
}

/// Regression test for diagnostics disappearing in some cases.
///
/// The specific way this fails is when a file that was never in the "open
/// file set" is closed. When that happens, there was a bug where the
/// open file set was completely cleared. This in turn would result in
/// `Project::should_check_file` returning `false` for any other open file. And
/// that would finally result in an empty set of diagnostics being returned,
/// which would effectively clear any existing diagnostics.
///
/// Moreover, since the file was no longer in the open set, there's likely
/// other mysterious failures happening.
///
/// See: <https://github.com/astral-sh/ty-vscode/issues/342>
#[test]
fn closing_external_file_preserves_open_files() -> Result<()> {
    let _filter = filter_result_id();

    let workspace_root = SystemPath::new("src");
    let main_path = SystemPath::new("src/main.py");
    let main_content = "\
def foo() -> str:
    return 42  # intentional type error to provoke some diagnostics
";

    let mut server = TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(main_path, main_content)?
        .build()
        .wait_until_workspaces_are_initialized();

    server.open_text_document(main_path, main_content, 1);
    let diagnostics_before = server.document_diagnostic_request(main_path, None);
    insta::assert_snapshot!(
        condensed_document_diagnostic_snapshot(diagnostics_before),
        @"1:11..1:13[ERROR]: Return type does not match returned value: expected `str`, found `Literal[42]`",
    );

    server.open_text_document("external/builtins.pyi", "class str: ...", 1);
    server.close_text_document("external/builtins.pyi");

    let diagnostics_after = server.document_diagnostic_request(main_path, None);
    insta::assert_snapshot!(
        condensed_document_diagnostic_snapshot(diagnostics_after),
        @"1:11..1:13[ERROR]: Return type does not match returned value: expected `str`, found `Literal[42]`",
    );

    Ok(())
}

#[test]
fn closing_deleted_file_clears_diagnostics_before_watcher_notification() -> Result<()> {
    let path = SystemPath::new("src/deleted.py");
    let source = "value: int = 'wrong'\n";
    let mut server = create_workspace_server_with_file(SystemPath::new("src"), path, source)?;
    server.open_text_document(path, source, 1);

    let initial = server.workspace_diagnostic_request(None, None);
    let previous_result_ids = extract_result_ids_from_response(&initial);
    insta::assert_snapshot!(condensed_workspace_diagnostic_snapshot(initial), @r#"
    file://<temp_dir>/src/deleted.py
    	0:13..0:20[ERROR]: Object of type `Literal["wrong"]` is not assignable to `int`
    "#);

    let request_id = server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
        previous_result_ids: previous_result_ids.clone(),
        ..WorkspaceDiagnosticParams::default()
    });
    assert_workspace_diagnostics_suspends_for_long_polling(&mut server, &request_id);

    std::fs::remove_file(server.file_path(path))?;
    server.close_text_document(path);
    insta::assert_snapshot!(
        condensed_document_diagnostic_snapshot(server.document_diagnostic_request(path, None)),
        @"",
    );

    let after_close = server.await_response::<WorkspaceDiagnosticRequest>(&request_id);
    insta::assert_snapshot!(
        condensed_workspace_diagnostic_snapshot(after_close),
        @"file://<temp_dir>/src/deleted.py",
    );

    server.did_change_watched_file(path, FileChangeType::Deleted);
    let after_watcher = server.workspace_diagnostic_request(None, Some(previous_result_ids));
    insta::assert_snapshot!(
        condensed_workspace_diagnostic_snapshot(after_watcher),
        @"file://<temp_dir>/src/deleted.py",
    );

    server.write_file(path, source)?;
    server.did_change_watched_file(path, FileChangeType::Created);
    let recreated = server.workspace_diagnostic_request(None, None);
    insta::assert_snapshot!(condensed_workspace_diagnostic_snapshot(recreated), @r#"
    file://<temp_dir>/src/deleted.py
    	0:13..0:20[ERROR]: Object of type `Literal["wrong"]` is not assignable to `int`
    "#);

    Ok(())
}

fn create_workspace_server_with_file(
    workspace_root: &SystemPath,
    file_path: &SystemPath,
    file_content: &str,
) -> Result<TestServer> {
    Ok(TestServerBuilder::new()?
        .with_workspace(workspace_root, None)?
        .with_file(file_path, file_content)?
        .with_initialization_options(
            &ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace),
        )
        .build()
        .wait_until_workspaces_are_initialized())
}

/// Sends a workspace diagnostic request to the server.
///
/// Unlike [`TestServer::workspace_diagnostic_request`], this function does not wait for the response.
pub(crate) fn send_workspace_diagnostic_request(server: &mut TestServer) -> lsp_server::RequestId {
    server.send_request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    })
}

pub(crate) fn shutdown_and_await_workspace_diagnostic(
    mut server: TestServer,
    request_id: &RequestId,
) -> WorkspaceDiagnosticReport {
    let shutdown_id = server.send_request::<lsp_types::ShutdownRequest>(());

    let workspace_response = server.await_response::<WorkspaceDiagnosticRequest>(request_id);

    server.await_response::<lsp_types::ShutdownRequest>(&shutdown_id);
    server.send_notification::<lsp_types::ExitNotification>(());

    workspace_response
}

#[track_caller]
pub(crate) fn assert_workspace_diagnostics_suspends_for_long_polling(
    server: &mut TestServer,
    request_id: &lsp_server::RequestId,
) {
    match server
        .try_await_response::<WorkspaceDiagnosticRequest>(request_id, Some(Duration::from_secs(2)))
    {
        Ok(_) => {
            panic!("Expected workspace diagnostic request to suspend for long-polling.");
        }
        Err(AwaitResponseError::Timeout) => {
        }
        Err(err) => {
            panic!(
                "Response should time out because the request is suspended for long-polling but errored with: {err}"
            )
        }
    }
}

fn extract_result_ids_from_response(response: &WorkspaceDiagnosticReport) -> Vec<PreviousResultId> {
    let items = &response.items;

    items
        .iter()
        .filter_map(|item| match item {
            WorkspaceDocumentDiagnosticReport::WorkspaceFullDocumentDiagnosticReport(
                full_report,
            ) => {
                let result_id = full_report
                    .full_document_diagnostic_report
                    .result_id
                    .as_ref()?;

                Some(PreviousResultId {
                    uri: full_report.uri.clone(),
                    value: result_id.clone(),
                })
            }
            WorkspaceDocumentDiagnosticReport::WorkspaceUnchangedDocumentDiagnosticReport(_) => {
                None
            }
        })
        .collect()
}

mod uv_metadata {
    use lsp_types::{
        Code, Position, Range, TextDocumentContentChangeEvent,
        TextDocumentContentChangeWholeDocument,
    };
    use ruff_db::diagnostic::DiagnosticId;
    use ruff_python_trivia::textwrap::dedent;

    use super::{
        ClientOptions, DiagnosticMode, DocumentDiagnosticReport, Result, SystemPath,
        TestServerBuilder, WorkspaceDocumentDiagnosticReport,
    };

    #[test]
    fn opening_new_file_updates_workspace_index() -> Result<()> {
        let added = SystemPath::new("src/added.py");
        let source = "missing\n";

        let mut server = TestServerBuilder::new()?
            .with_workspace(
                SystemPath::new("src"),
                Some(ClientOptions::default().with_diagnostic_mode(DiagnosticMode::Workspace)),
            )?
            .with_file(SystemPath::new("src/initial.py"), source)?
            .with_uv_script_manager()
            .build()
            .wait_until_workspaces_are_initialized();

        let _ = server.workspace_diagnostic_request(None, None);
        server.write_file(added, source)?;
        server.open_text_document(added, source, 1);

        let uri = server.file_uri(added);
        let diagnostics = server.workspace_diagnostic_request(None, None);
        assert!(diagnostics.items.iter().any(|report| matches!(
            report,
            WorkspaceDocumentDiagnosticReport::WorkspaceFullDocumentDiagnosticReport(report)
                if report.uri == uri
        )));

        Ok(())
    }

    #[test]
    fn synchronization_failure_highlights_script_metadata() -> Result<()> {
        let workspace_root = SystemPath::new("src");
        let script = SystemPath::new("src/script.py");
        let source = dedent(
            r"
            #!/usr/bin/env python3

            # /// script
            # [tool.ty]
            # project-manager.script = 'uv'
            # ///

            value = 1
            ",
        );
        let source = source.trim_start();

        let mut server = TestServerBuilder::new()?
            .with_workspace(workspace_root, None)?
            .with_file(script, source)?
            .with_env_var("UV", "missing-ty-script-uv-executable")
            .enable_workspace_diagnostic_refresh(true)
            .build()
            .wait_until_workspaces_are_initialized();

        server.open_text_document(script, source, 1);
        server.await_diagnostic_refresh();
        let report = server.document_diagnostic_request(script, None);
        let DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) = report else {
            anyhow::bail!("expected a full diagnostic report for the script");
        };

        let diagnostic = report
            .full_document_diagnostic_report
            .items
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(Code::String(DiagnosticId::ProjectManager.as_str().into()))
            })
            .ok_or_else(|| {
                anyhow::anyhow!("expected the script synchronization error: {report:?}")
            })?;

        assert_eq!(
            diagnostic.range,
            Range::new(Position::new(2, 0), Position::new(5, 5))
        );

        Ok(())
    }

    #[test]
    fn unsaved_script_uses_its_settings_and_keeps_diagnostics() -> Result<()> {
        let workspace_root = SystemPath::new("src");
        let script = SystemPath::new("src/script.py");
        let initial = "PythonFinalizationError\nmissing\n";
        let updated = dedent(
            r"
            # /// script
            # requires-python = '>=3.13'
            # dependencies = []
            #
            # [tool.ty]
            # project-manager.script = 'uv'
            # ///

            PythonFinalizationError
            missing
            ",
        );

        let mut server = TestServerBuilder::new()?
            .with_workspace(workspace_root, None)?
            .with_file(script, initial)?
            .with_file(
                SystemPath::new("src/ty.toml"),
                dedent(
                    r"
                    [environment]
                    python-version = '3.12'
                    ",
                ),
            )?
            .with_env_var("UV", "missing-ty-script-uv-executable")
            .enable_workspace_diagnostic_refresh(true)
            .build()
            .wait_until_workspaces_are_initialized();

        server.open_text_document(script, initial, 1);

        let report = server.document_diagnostic_request(script, None);
        let DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) = report else {
            anyhow::bail!("expected a full diagnostic report for the ordinary file");
        };
        assert_eq!(report.full_document_diagnostic_report.items.len(), 2);

        server.change_text_document(
            script,
            vec![
                TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                    TextDocumentContentChangeWholeDocument {
                        text: updated.to_string(),
                    },
                ),
            ],
            2,
        );

        let report = server.document_diagnostic_request(script, None);
        let DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) = report else {
            anyhow::bail!("expected a full diagnostic report for the provisional script");
        };
        let [diagnostic] = report.full_document_diagnostic_report.items.as_slice() else {
            anyhow::bail!("expected only the unresolved `missing` reference");
        };
        assert_eq!(
            diagnostic.code,
            Some(Code::String("unresolved-reference".to_string()))
        );
        assert_eq!(diagnostic.range.start.line, 10);

        server.write_file(script, updated)?;
        server.save_text_document(script);
        server.await_diagnostic_refresh();

        let report = server.document_diagnostic_request(script, None);
        let DocumentDiagnosticReport::RelatedFullDocumentDiagnosticReport(report) = report else {
            anyhow::bail!("expected a full diagnostic report for the synchronized script");
        };
        assert!(
            report
                .full_document_diagnostic_report
                .items
                .iter()
                .any(|diagnostic| diagnostic.code
                    == Some(Code::String(DiagnosticId::ProjectManager.as_str().into())))
        );

        Ok(())
    }
}
