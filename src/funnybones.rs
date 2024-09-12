//! The FunnyBones 2D Animation Editor.

use std::path::PathBuf;

use cushy::{
    kludgine::app::winit::keyboard::ModifiersState,
    value::{Destination, Dynamic, DynamicRead},
    widget::{MakeWidget, SharedCallback, HANDLED},
    widgets::layers::Modal,
    window::{MakeWindow, PendingWindow, Window, WindowHandle},
    App, Application, ModifiersStateExt, Open, PendingApp, WithClone,
};
use funnybones::editor::{EditingSkeleton, SaveError};

fn main() -> cushy::Result {
    let pending_app = PendingApp::default();

    main_menu_window(&pending_app).run_centered_in(pending_app)
}

fn skeleton_window(path: Option<PathBuf>) -> Window {
    let modals = Modal::new();
    let editing_skeleton = if let Some(path) = path.as_ref() {
        match EditingSkeleton::read_from(path) {
            Ok(skeleton) => skeleton,
            Err(err) => return err.to_string().centered().pad().into_window(),
        }
    } else {
        EditingSkeleton::default()
    };
    let path = Dynamic::new(path);

    let on_error = SharedCallback::new(|err: SaveError| {
        todo!("show {err}");
    });
    let skeleton_editor = funnybones::editor::skeleton_editor(editing_skeleton.clone());

    skeleton_editor
        .expand()
        .and(modals.clone())
        .into_layers()
        .with_shortcut("s", ModifiersState::PRIMARY, {
            (&path, &editing_skeleton, &on_error, &modals).with_clone(
                |(path, editing_skeleton, on_error, modals)| {
                    move |_| {
                        if let Err(err) = save(&path, &editing_skeleton, &on_error, &modals) {
                            on_error.invoke(err);
                        }
                        HANDLED
                    }
                },
            )
        })
        .with_shortcut("s", ModifiersState::PRIMARY | ModifiersState::SHIFT, {
            move |_| {
                save_as(&path, &editing_skeleton, &on_error, &modals);
                HANDLED
            }
        })
        .into_window()
}

fn save(
    path: &Dynamic<Option<PathBuf>>,
    skeleton: &EditingSkeleton,
    on_error: &SharedCallback<SaveError>,
    modals: &Modal,
) -> Result<(), SaveError> {
    let current_path = path.read();
    if let Some(path) = &*current_path {
        skeleton.write_to(path)
    } else {
        save_as(path, skeleton, on_error, modals);
        Ok(())
    }
}

fn save_as(
    path: &Dynamic<Option<PathBuf>>,
    skeleton: &EditingSkeleton,
    on_error: &SharedCallback<SaveError>,
    modals: &Modal,
) {
    (path, skeleton, on_error, modals).with_clone(|(path, skeleton, on_error, modals)| {
        std::thread::spawn(move || {
            modals.present("Please dismiss the save file dialog to continue editing.");
            let new_path = rfd::FileDialog::new()
                .add_filter("FunnyBones Skeleton (.fbs)", &["fbs"])
                .save_file();
            modals.dismiss();
            if let Some(new_path) = new_path {
                match skeleton.write_to(&new_path) {
                    Ok(()) => {
                        path.set(Some(new_path));
                    }
                    Err(err) => on_error.invoke(err),
                }
            }
        });
    });
}

fn main_menu_window(app: &impl Application) -> Window {
    let window = PendingWindow::default();
    let handle = window.handle();
    let visible = Dynamic::new(true);

    window
        .with_root(
            "New Skeleton"
                .into_button()
                .on_click({
                    let mut app = app.as_app();
                    let handle = handle.clone();
                    move |_| {
                        let _ = skeleton_window(None).open(&mut app);
                        handle.request_close();
                    }
                })
                .and("New Animation".into_button())
                .and("Open Existing...".into_button().on_click({
                    let mut app = app.as_app();
                    let handle = handle.clone();
                    let visible = visible.clone();
                    move |_| {
                        visible.set(false);
                        open_file(&mut app, &handle, true);
                    }
                }))
                .into_rows()
                .pad(),
        )
        .resize_to_fit(true)
        .resizable(false)
        .visible(visible)
}

fn open_file(app: &mut App, parent_window: &WindowHandle, close: bool) {
    parent_window.execute({
        let mut app = app.clone();
        let parent_window = parent_window.clone();
        move |context| {
            let dialog = rfd::FileDialog::new()
                .add_filter("FunnyBones Skeleton (.fbs)", &["fbs"])
                .set_parent(context.winit().expect("running on winit"));
            std::thread::spawn(move || {
                if let Some(file) = dialog.pick_file() {
                    if file.extension().map_or(false, |ext| ext == "fbs") {
                        let _ = skeleton_window(Some(file)).open(&mut app);
                    } else {
                        todo!("unknown file type");
                    }
                }
                if close {
                    parent_window.request_close();
                }
            });
        }
    });
}
