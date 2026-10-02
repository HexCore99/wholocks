mod view;

use std::{collections::HashSet, io, path::PathBuf};

use gpui::{
    App, AppContext as _, Application, Bounds, Context, PromptButton, PromptLevel, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, px, size,
};

use crate::{LockingProcess, find_locks, is_elevated, relaunch_elevated, terminate_process};

const ERROR_ACCESS_DENIED: i32 = 5;

pub fn run(target: PathBuf) {
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(780.0), px(520.0)), cx);
        let title = target
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("WhoLocks — {name}"))
            .unwrap_or_else(|| "WhoLocks".to_string());

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(title)),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(640.0), px(360.0))),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|cx| {
                    let mut view = WhoLocksView::new(target, is_elevated());
                    view.refresh(cx);
                    view
                })
            },
        )
        .expect("failed to open the WhoLocks window");

        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
}

pub(crate) struct WhoLocksView {
    target: PathBuf,
    processes: Vec<LockingProcess>,
    inaccessible_process_count: usize,
    loading: bool,
    scan_error: Option<String>,
    action_message: Option<ActionMessage>,
    busy_pids: HashSet<u32>,
    bulk_busy: bool,
    elevated: bool,
    needs_elevation: bool,
    termination_access_denied: bool,
    scan_generation: u64,
}

pub(crate) struct ActionMessage {
    pub(crate) is_error: bool,
    pub(crate) text: String,
}

impl WhoLocksView {
    fn new(target: PathBuf, elevated: bool) -> Self {
        Self {
            target,
            processes: Vec::new(),
            inaccessible_process_count: 0,
            loading: false,
            scan_error: None,
            action_message: None,
            busy_pids: HashSet::new(),
            bulk_busy: false,
            elevated,
            needs_elevation: false,
            termination_access_denied: false,
            scan_generation: 0,
        }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.scan_generation = self.scan_generation.wrapping_add(1);
        let generation = self.scan_generation;
        let target = self.target.clone();
        self.loading = true;
        self.scan_error = None;
        cx.notify();

        let scan = cx
            .background_executor()
            .spawn(async move { find_locks(&target) });
        cx.spawn(async move |view, cx| {
            let result = scan.await;
            view.update(cx, |view, cx| {
                if view.scan_generation != generation {
                    return;
                }

                view.loading = false;
                match result {
                    Ok(report) => {
                        view.processes = report.processes;
                        view.inaccessible_process_count = report.inaccessible_process_count;
                        if view.processes.is_empty() {
                            view.termination_access_denied = false;
                        }
                        view.needs_elevation =
                            report.inaccessible_process_count > 0 || view.termination_access_denied;
                    }
                    Err(error) => {
                        view.processes.clear();
                        view.inaccessible_process_count = 0;
                        view.scan_error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn confirm_end_task(
        &mut self,
        pid: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(process) = self.processes.iter().find(|process| process.pid == pid) else {
            return;
        };
        let name = process
            .executable
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown process");
        let detail =
            format!("{name} (PID {pid}) will be stopped immediately. Unsaved work may be lost.");
        let answer = window.prompt(
            PromptLevel::Warning,
            "End this task?",
            Some(&detail),
            &[PromptButton::ok("End Task"), PromptButton::cancel("Cancel")],
            cx,
        );

        cx.spawn(async move |view, cx| {
            if answer.await.ok() == Some(0) {
                view.update(cx, |view, cx| view.end_tasks(vec![pid], false, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub(crate) fn confirm_end_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pids: Vec<_> = self
            .processes
            .iter()
            .map(|process| process.pid)
            .filter(|pid| !self.busy_pids.contains(pid))
            .collect();
        if pids.is_empty() {
            return;
        }

        let detail = format!(
            "{} processes will be stopped immediately. Unsaved work may be lost.",
            pids.len()
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "End all locking tasks?",
            Some(&detail),
            &[
                PromptButton::ok("End All Tasks"),
                PromptButton::cancel("Cancel"),
            ],
            cx,
        );

        cx.spawn(async move |view, cx| {
            if answer.await.ok() == Some(0) {
                view.update(cx, |view, cx| view.end_tasks(pids, true, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn end_tasks(&mut self, pids: Vec<u32>, bulk: bool, cx: &mut Context<Self>) {
        if pids.is_empty() {
            return;
        }
        self.action_message = None;
        self.termination_access_denied = false;
        self.bulk_busy = bulk;
        self.busy_pids.extend(pids.iter().copied());
        cx.notify();

        let terminate = cx.background_executor().spawn(async move {
            pids.into_iter()
                .map(|pid| {
                    let result = terminate_process(pid);
                    let access_denied = result.as_ref().err().and_then(io::Error::raw_os_error)
                        == Some(ERROR_ACCESS_DENIED);
                    (
                        pid,
                        result.map_err(|error| error.to_string()),
                        access_denied,
                    )
                })
                .collect::<Vec<_>>()
        });

        cx.spawn(async move |view, cx| {
            let results = terminate.await;
            view.update(cx, |view, cx| {
                view.bulk_busy = false;
                let mut failures = Vec::new();
                for (pid, result, access_denied) in results {
                    view.busy_pids.remove(&pid);
                    if access_denied {
                        view.termination_access_denied = true;
                        view.needs_elevation = true;
                    }
                    if let Err(error) = result {
                        failures.push(format!("PID {pid}: {error}"));
                    }
                }

                view.action_message = Some(if failures.is_empty() {
                    ActionMessage {
                        is_error: false,
                        text: "Task termination completed.".to_string(),
                    }
                } else {
                    ActionMessage {
                        is_error: true,
                        text: format!("Could not end {}", failures.join("; ")),
                    }
                });
                view.refresh(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn retry_as_administrator(&mut self, cx: &mut Context<Self>) {
        match relaunch_elevated(&self.target) {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.action_message = Some(ActionMessage {
                    is_error: true,
                    text: format!("Administrator restart was cancelled or failed: {error}"),
                });
                cx.notify();
            }
        }
    }
}
