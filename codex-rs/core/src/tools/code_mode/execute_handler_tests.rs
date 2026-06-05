use super::*;
use crate::session::tests::make_session_and_context;
use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::context::ToolInvocation;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PreToolUsePayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

fn sample_source() -> &'static str {
    "const r = await tools.exec_command({cmd:\"echo hi\"});\ntext(r.output);"
}

fn handler() -> CodeModeExecuteHandler {
    // `pre_tool_use_payload` does not consult `self.spec` or nested specs; an
    // empty Code Mode tool spec is sufficient for hook-payload behavior tests.
    let spec = create_code_mode_tool(
        &[],
        &BTreeMap::new(),
        /*code_mode_only*/ true,
        /*deferred_tools_available*/ false,
    );
    CodeModeExecuteHandler::new(spec, Vec::new())
}

async fn invocation_for_payload(payload: ToolPayload) -> ToolInvocation {
    let (session, turn) = make_session_and_context().await;
    ToolInvocation {
        session: session.into(),
        turn: turn.into(),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
        call_id: "call-code-mode".to_string(),
        tool_name: codex_tools::ToolName::plain("exec"),
        source: crate::tools::context::ToolCallSource::Direct,
        payload,
    }
}

#[tokio::test]
async fn pre_tool_use_payload_uses_freeform_code_mode_input() {
    let source = sample_source();
    let payload = ToolPayload::Custom {
        input: source.to_string(),
    };
    let invocation = invocation_for_payload(payload).await;

    assert_eq!(
        handler().pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::code_mode_exec(),
            tool_input: json!({ "command": source }),
        })
    );
}

#[tokio::test]
async fn pre_tool_use_payload_returns_none_for_non_custom_payload() {
    let payload = ToolPayload::Function {
        arguments: "{}".to_string(),
    };
    let invocation = invocation_for_payload(payload).await;

    assert_eq!(handler().pre_tool_use_payload(&invocation), None);
}

#[tokio::test]
async fn with_updated_hook_input_rewrites_freeform_code_mode_input() -> anyhow::Result<()> {
    let invocation = invocation_for_payload(ToolPayload::Custom {
        input: sample_source().to_string(),
    })
    .await;
    let rewritten_source = "text('rewritten by hook');";

    let invocation =
        handler().with_updated_hook_input(invocation, json!({ "command": rewritten_source }))?;

    let ToolPayload::Custom { input } = invocation.payload else {
        panic!("rewritten Code Mode input should remain a custom payload");
    };
    assert_eq!(input, rewritten_source);
    Ok(())
}

#[tokio::test]
async fn with_updated_hook_input_requires_string_command() {
    let invocation = invocation_for_payload(ToolPayload::Custom {
        input: sample_source().to_string(),
    })
    .await;

    let err = match handler().with_updated_hook_input(invocation, json!({ "command": 12 })) {
        Ok(_) => panic!("non-string command should be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.to_string(),
        "hook returned updatedInput without string field `command`"
    );
}
