//! GPUI owns macOS authorization, foreground presentation and native delegate lifetime.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, SystemNotification, SystemNotificationAction};

use super::{ToastAction, ToastActivation};

const CAPACITY: usize = 64;
static SEND: OnceLock<Mutex<mpsc::Sender<Pending>>> = OnceLock::new();

struct Pending {
    title: String,
    body: String,
    activation: Option<ToastActivation>,
    actions: Vec<ToastAction>,
}

struct Callbacks {
    tag: String,
    activation: Option<ToastActivation>,
    actions: Vec<ToastAction>,
}

pub(crate) fn init(cx: &mut App) {
    // The pinned GPUI backend aborts if UNUserNotificationCenter is used outside a bundle.
    if objc2_foundation::NSBundle::mainBundle().bundleIdentifier().is_none() {
        return;
    }
    let (sender, mut receiver) = mpsc::channel::<Pending>(CAPACITY);
    if SEND.set(Mutex::new(sender)).is_err() {
        return;
    }
    let callbacks = Rc::new(RefCell::new(VecDeque::<Callbacks>::new()));
    let response_callbacks = callbacks.clone();
    cx.on_system_notification_response(move |response, cx| {
        let callback = {
            let mut callbacks = response_callbacks.borrow_mut();
            let Some(index) = callbacks.iter().position(|item| item.tag.as_str() == &*response.tag)
            else {
                return;
            };
            callbacks.remove(index).unwrap()
        };
        // Release the registry borrow before activation can re-enter application delivery.
        cx.dismiss_system_notification(&callback.tag);
        if let Some(id) = response.action_id {
            if let Some(index) =
                id.strip_prefix("choice-").and_then(|value| value.parse::<usize>().ok())
                && let Some(action) = callback.actions.get(index)
            {
                (action.activate)();
            }
        } else if let Some(activate) = callback.activation {
            activate();
        }
    });
    cx.spawn(async move |cx| {
        let mut serial = 0_u64;
        while let Some(pending) = receiver.next().await {
            serial = serial.wrapping_add(1);
            let tag = format!("pebrel-notification-{serial}");
            cx.update(|cx| {
                let actions = pending
                    .actions
                    .iter()
                    .enumerate()
                    .map(|(index, action)| SystemNotificationAction {
                        id: format!("choice-{index}").into(),
                        label: action.label.clone().into(),
                    })
                    .collect();
                {
                    let mut callbacks = callbacks.borrow_mut();
                    if callbacks.len() == CAPACITY {
                        let expired = callbacks.pop_front().unwrap();
                        cx.dismiss_system_notification(&expired.tag);
                    }
                    callbacks.push_back(Callbacks {
                        tag: tag.clone(),
                        activation: pending.activation,
                        actions: pending.actions,
                    });
                }
                cx.show_system_notification(SystemNotification {
                    tag: tag.into(),
                    title: pending.title.into(),
                    body: pending.body.into(),
                    actions,
                });
            });
        }
    })
    .detach();
}

pub(super) fn dispatch(
    title: String,
    body: String,
    activation: Option<ToastActivation>,
    actions: Vec<ToastAction>,
) {
    let Some(sender) = SEND.get() else {
        log::warn!("System notifications require a registered Pebrel application bundle");
        return;
    };
    // No worker, RPC wait or blocking send on terminal/UI threads.
    let Ok(mut sender) = sender.try_lock() else {
        log::warn!("notify: native notification queue busy");
        return;
    };
    if let Err(error) = sender.try_send(Pending { title, body, activation, actions }) {
        log::warn!("notify: native notification queue unavailable: {error}");
    }
}
