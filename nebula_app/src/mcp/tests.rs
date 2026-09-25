use super::*;
use futures::StreamExt;
use serde_json::json;

#[test]
fn tool_contract_rejects_target_injection_and_malformed_input() {
    for value in [
        json!({"operation":"run","command":"echo ok","pane_id":2}),
        json!({"operation":"read","lines":5000}),
        json!({"operation":"input","text":"echo ok","key":"enter"}),
        json!({"operation":"run","command":"echo ok\nwhoami"}),
        json!({"operation":"input","text":"\u{1b}[20h"}),
    ] {
        assert!(
            serde_json::from_value::<Operation>(value)
                .map(|op| op.validate().is_err())
                .unwrap_or(true)
        );
    }
    let read: Operation = serde_json::from_value(json!({"operation":"read"})).unwrap();
    assert!(!read.is_write());
    let input: Operation =
        serde_json::from_value(json!({"operation":"input","key":"c","modifiers":{"control":true}}))
            .unwrap();
    assert!(input.is_write());
    assert!(input.validate().is_ok());
}

#[tokio::test]
async fn approval_is_single_use_and_cannot_resolve_another_call() {
    let (share, mut receiver) = Share::new(1, &CancellationToken::new()).unwrap();
    let task = tokio::spawn({
        let share = share.clone();
        async move {
            share
                .invoke(
                    Operation::Run { command: "echo approved".into() },
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let call = receiver.next().await.unwrap();
    let id = call.id;
    let mut pending = Some((call, 7));
    assert!(!task.is_finished());
    assert!(take_approval(&mut pending, id + 1, 7).is_none());
    assert!(pending.is_some());
    let call = take_approval(&mut pending, id, 7).unwrap();
    call.respond(Ok(json!({"submitted":true})));
    assert!(take_approval(&mut pending, id, 7).is_none());
    assert!(task.await.unwrap().is_ok());
}

#[tokio::test]
async fn changed_input_and_revocation_never_execute_pending_writes() {
    for revoke in [false, true] {
        let (share, mut receiver) = Share::new(1, &CancellationToken::new()).unwrap();
        let task = tokio::spawn({
            let share = share.clone();
            async move {
                share
                    .invoke(Operation::Run { command: "echo no".into() }, &CancellationToken::new())
                    .await
            }
        });
        let call = receiver.next().await.unwrap();
        let id = call.id;
        let mut pending = Some((call, 7));
        if revoke {
            share.stop();
        }
        assert!(take_approval(&mut pending, id, if revoke { 7 } else { 8 }).is_none());
        assert!(task.await.unwrap().is_err());
    }
}

#[tokio::test]
async fn client_cancellation_and_single_writer_gate() {
    let (share, mut receiver) = Share::new(1, &CancellationToken::new()).unwrap();
    let client = CancellationToken::new();
    let task = tokio::spawn({
        let share = share.clone();
        let client = client.clone();
        async move { share.invoke(Operation::Run { command: "echo pending".into() }, &client).await }
    });
    let call = receiver.next().await.unwrap();
    let concurrent = share
        .invoke(Operation::Run { command: "echo second".into() }, &CancellationToken::new())
        .await;
    assert!(concurrent.is_err());
    client.cancel();
    assert!(task.await.unwrap().is_err());
    assert!(!call.is_live());
}

#[test]
fn loopback_endpoint_requires_its_own_token() {
    let host = Host::start().unwrap();
    let (a, _) = host.share().unwrap();
    let (b, _) = host.share().unwrap();
    assert_ne!(a.token, b.token);
    assert_ne!(a.url, b.url);
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});
    let send = |token: &str, origin: bool| {
        let mut request = ureq::post(&a.url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json, text/event-stream");
        if origin {
            request = request.header("Origin", "http://attacker.invalid");
        }
        request.send_json(&init).map(|response| response.status().as_u16())
    };
    assert!(matches!(send(&b.token, false), Err(ureq::Error::StatusCode(401))));
    assert!(matches!(send(&a.token, true), Err(ureq::Error::StatusCode(403))));
    assert_eq!(send(&a.token, false).unwrap(), 200);
    a.stop();
    assert!(matches!(send(&a.token, false), Err(ureq::Error::StatusCode(410))));
}
