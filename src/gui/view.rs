use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb};

use super::WhoLocksView;
use crate::LockingProcess;

fn neutral_button(label: &str) -> gpui::Div {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0xe5e7eb))
        .text_color(rgb(0x111827))
        .text_sm()
        .cursor_pointer()
        .child(label.to_string())
}

fn danger_button(label: &str) -> gpui::Div {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0xdc2626))
        .text_color(rgb(0xffffff))
        .text_sm()
        .cursor_pointer()
        .child(label.to_string())
}

impl WhoLocksView {
    fn process_row(&self, process: &LockingProcess, cx: &mut Context<Self>) -> gpui::Div {
        let pid = process.pid;
        let busy = self.busy_pids.contains(&pid);
        let name = process
            .executable
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown process")
            .to_string();
        let executable = process.executable.display().to_string();
        let handle_count = process.matched_handles.len();

        let action = if busy {
            neutral_button("Ending...").id(("ending-task", pid as usize))
        } else {
            danger_button("End Task")
                .id(("end-task", pid as usize))
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.confirm_end_task(pid, window, cx);
                }))
        };

        div()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .py_3()
            .border_1()
            .border_color(rgb(0xe5e7eb))
            .rounded_md()
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(1.0))
                    .gap_1()
                    .child(div().text_sm().text_color(rgb(0x111827)).child(name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x6b7280))
                            .overflow_hidden()
                            .child(executable),
                    )
                    .child(div().text_xs().text_color(rgb(0x9ca3af)).child(format!(
                        "{handle_count} matching handle{}",
                        if handle_count == 1 { "" } else { "s" }
                    ))),
            )
            .child(
                div()
                    .w(px(90.0))
                    .text_sm()
                    .text_color(rgb(0x374151))
                    .child(pid.to_string()),
            )
            .child(div().w(px(110.0)).flex().justify_end().child(action))
    }
}

impl Render for WhoLocksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_end_all = !self.loading
            && !self.bulk_busy
            && !self.processes.is_empty()
            && self.busy_pids.is_empty();
        let end_all = if can_end_all {
            danger_button("End All Tasks")
                .id("end-all-tasks")
                .on_click(cx.listener(|view, _, window, cx| {
                    view.confirm_end_all(window, cx);
                }))
        } else {
            neutral_button(if self.bulk_busy {
                "Ending tasks..."
            } else {
                "End All Tasks"
            })
            .id("end-all-disabled")
        };

        let mut content = div().flex().flex_col().flex_1().min_h(px(1.0));
        if self.loading {
            content = content.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0x6b7280))
                    .child("Scanning Windows handles..."),
            );
        } else if let Some(error) = &self.scan_error {
            content = content.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(rgb(0x991b1b))
                    .child("The selected path could not be scanned.")
                    .child(div().text_sm().child(error.clone())),
            );
        } else if self.processes.is_empty() {
            content = content.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .text_color(rgb(0x166534))
                            .child("No locking processes found"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x6b7280))
                            .child("The file or folder is not currently held open."),
                    ),
            );
        } else {
            content = content
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .px_4()
                        .pb_2()
                        .text_xs()
                        .text_color(rgb(0x6b7280))
                        .child(div().flex_1().child("USED BY"))
                        .child(div().w(px(90.0)).child("PID"))
                        .child(div().w(px(110.0)).text_right().child("ACTION")),
                )
                .child(
                    div()
                        .id("process-list")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(1.0))
                        .gap_2()
                        .overflow_y_scroll()
                        .children(
                            self.processes
                                .iter()
                                .map(|process| self.process_row(process, cx)),
                        ),
                );
        }

        let mut root =
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .p_5()
                .bg(rgb(0xf8fafc))
                .text_color(rgb(0x111827))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(1.0))
                                .gap_1()
                                .child(div().text_2xl().child("Who locks this?"))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0x6b7280))
                                        .overflow_hidden()
                                        .child(self.target.display().to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(neutral_button("Refresh").id("refresh").on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.action_message = None;
                                        view.refresh(cx);
                                    }),
                                ))
                                .child(end_all),
                        ),
                );

        if self.inaccessible_process_count > 0 || (!self.elevated && self.needs_elevation) {
            let warning_text = if self.inaccessible_process_count > 0 {
                "Some processes could not be inspected. If a locking process is missing, retry as administrator."
                    .to_string()
            } else {
                "Administrator permission is needed to end one or more tasks.".to_string()
            };
            let mut warning = div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0xfef3c7))
                .text_sm()
                .text_color(rgb(0x92400e))
                .child(warning_text);

            if !self.elevated && self.needs_elevation {
                warning = warning.child(
                    neutral_button("Retry as administrator")
                        .id("retry-as-admin")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.retry_as_administrator(cx);
                        })),
                );
            }
            root = root.child(warning);
        }

        if let Some(message) = &self.action_message {
            root = root.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if message.is_error {
                        rgb(0xfee2e2)
                    } else {
                        rgb(0xdcfce7)
                    })
                    .text_sm()
                    .text_color(if message.is_error {
                        rgb(0x991b1b)
                    } else {
                        rgb(0x166534)
                    })
                    .child(message.text.clone()),
            );
        }

        root.child(content)
    }
}
