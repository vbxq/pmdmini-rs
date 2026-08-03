// SPDX-License-Identifier: AGPL-3.0-only

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use eframe::egui;
use js_sys::Uint8Array;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{DragEvent, Event, File, FileList, HtmlInputElement};

use crate::bundle::MemoryBundle;

type PendingBundles = Rc<RefCell<VecDeque<Result<MemoryBundle, String>>>>;

pub struct WebFilePicker {
    input: HtmlInputElement,
    pending: PendingBundles,
    _change_listener: Closure<dyn FnMut(Event)>,
    _dragover_listener: Closure<dyn FnMut(DragEvent)>,
    _drop_listener: Closure<dyn FnMut(DragEvent)>,
}

impl WebFilePicker {
    pub fn new(repaint: egui::Context) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window is unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("document is unavailable"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("document body is unavailable"))?;

        let input = document
            .create_element("input")?
            .dyn_into::<HtmlInputElement>()?;
        input.set_type("file");
        input.set_multiple(true);
        input.set_accept(".M,.M2,.MZ,.PPC,.PPS,.PZI,.PVI,.P86");
        input.style().set_property("display", "none")?;
        body.append_child(&input)?;

        let pending = Rc::new(RefCell::new(VecDeque::new()));

        let change_pending = Rc::clone(&pending);
        let change_input = input.clone();
        let change_repaint = repaint.clone();
        let change_listener = Closure::wrap(Box::new(move |_event: Event| {
            if let Some(files) = change_input.files() {
                enqueue_file_list(Rc::clone(&change_pending), change_repaint.clone(), &files);
            }

            change_input.set_value("");
        }) as Box<dyn FnMut(Event)>);
        input
            .add_event_listener_with_callback("change", change_listener.as_ref().unchecked_ref())?;

        let dragover_listener = Closure::wrap(Box::new(move |event: DragEvent| {
            event.prevent_default();
            if let Some(data_transfer) = event.data_transfer() {
                data_transfer.set_drop_effect("copy");
            }
        }) as Box<dyn FnMut(DragEvent)>);
        document.add_event_listener_with_callback(
            "dragover",
            dragover_listener.as_ref().unchecked_ref(),
        )?;

        let drop_pending = Rc::clone(&pending);
        let drop_repaint = repaint;
        let drop_listener = Closure::wrap(Box::new(move |event: DragEvent| {
            event.prevent_default();
            let Some(data_transfer) = event.data_transfer() else {
                return;
            };
            if let Some(files) = data_transfer.files() {
                enqueue_file_list(Rc::clone(&drop_pending), drop_repaint.clone(), &files);
            }
        }) as Box<dyn FnMut(DragEvent)>);
        document
            .add_event_listener_with_callback("drop", drop_listener.as_ref().unchecked_ref())?;

        Ok(Self {
            input,
            pending,
            _change_listener: change_listener,
            _dragover_listener: dragover_listener,
            _drop_listener: drop_listener,
        })
    }

    pub fn open(&self) {
        self.input.click();
    }

    pub fn poll(&self) -> Option<Result<MemoryBundle, String>> {
        self.pending.borrow_mut().pop_front()
    }
}

fn enqueue_file_list(pending: PendingBundles, repaint: egui::Context, files: &FileList) {
    let mut selected = Vec::new();
    for index in 0..files.length() {
        if let Some(file) = files.item(index) {
            selected.push(file);
        }
    }
    if selected.is_empty() {
        return;
    }
    spawn_local(read_files(pending, repaint, selected));
}

async fn read_files(pending: PendingBundles, repaint: egui::Context, files: Vec<File>) {
    let result = read_files_inner(files).await;
    pending.borrow_mut().push_back(result);
    repaint.request_repaint();
}

async fn read_files_inner(files: Vec<File>) -> Result<MemoryBundle, String> {
    let mut named_bytes = Vec::with_capacity(files.len());
    for file in files {
        let buffer = JsFuture::from(file.array_buffer())
            .await
            .map_err(|error| format_js_error("read browser file", error))?;
        let bytes = Uint8Array::new(&buffer).to_vec();
        named_bytes.push((file.name(), bytes));
    }
    MemoryBundle::from_named_bytes(named_bytes)
}

fn format_js_error(operation: &str, error: JsValue) -> String {
    format!("{operation}: {error:?}")
}
