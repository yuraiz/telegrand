use std::cell::RefCell;

use glib::clone;
use glib::subclass::Signal;
use glib::WeakRef;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;
use tdlib::enums::FormattedText as EnumFormattedText;
use tdlib::functions;
use tdlib::types::FormattedText;

use crate::tdlib::BoxedFormattedText;
use crate::tdlib::Chat;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/components-message-entry.ui")]
    pub(crate) struct MessageEntry {
        pub(super) chat: WeakRef<Chat>,
        pub(super) formatted_text: RefCell<Option<BoxedFormattedText>>,
        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub(super) placeholder: TemplateChild<gtk::Inscription>,
        #[template_child]
        pub(super) emoji_button: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_view: TemplateChild<gtk::TextView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageEntry {
        const NAME: &'static str = "MessageEntry";
        type Type = super::MessageEntry;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageEntry {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("activate").build(),
                    Signal::builder("paste-clipboard").build(),
                    Signal::builder("emoji-button-press")
                        .param_types([gtk::Image::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<BoxedFormattedText>("formatted-text")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecString::builder("placeholder-text")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<Chat>("chat")
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "formatted-text" => obj.set_formatted_text(value.get().unwrap()),
                "placeholder-text" => obj.set_placeholder_text(value.get().unwrap()),
                "chat" => obj.set_chat(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "formatted-text" => obj.formatted_text().to_value(),
                "placeholder-text" => obj.placeholder_text().to_value(),
                "chat" => obj.chat().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.placeholder
                .connect_text_notify(clone!(@weak obj => move |_| obj.notify("placeholder-text")));

            // Handle the enter key to emit the "activate" signal if neither the "ctrl" nor the
            // "shift" modifier are pressed at the same time.
            let key_events = gtk::EventControllerKey::new();
            key_events.connect_key_pressed(
                clone!(@weak obj => @default-return gtk::Inhibit(false), move |_, key, _, modifier| {
                    gtk::Inhibit(
                        if !modifier.contains(gdk::ModifierType::CONTROL_MASK)
                            && !modifier.contains(gdk::ModifierType::SHIFT_MASK)
                            && (key == gdk::Key::Return || key == gdk::Key::KP_Enter)
                        {
                            obj.emit_by_name::<()>("activate", &[]);
                            true
                        } else {
                            false
                        },
                    )
                }),
            );
            self.text_view.add_controller(key_events);

            let press = gtk::GestureClick::new();
            press.connect_pressed(clone!(@weak obj => move |_, _, _, _| {
                obj.emit_by_name::<()>("emoji-button-press", &[&*obj.imp().emoji_button]);
            }));
            self.emoji_button.add_controller(press);

            self.text_view
                .buffer()
                .connect_changed(clone!(@weak obj => move |_| {
                    obj.text_buffer_changed();
                }));

            self.text_view
                .connect_paste_clipboard(clone!(@weak obj => move |_| {
                    obj.emit_by_name::<()>("paste-clipboard", &[]);
                }));
        }

        fn dispose(&self) {
            self.overlay.unparent();
        }
    }

    impl WidgetImpl for MessageEntry {
        fn grab_focus(&self) -> bool {
            self.text_view.grab_focus()
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageEntry(ObjectSubclass<imp::MessageEntry>)
        @extends gtk::Widget;
}

impl MessageEntry {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    fn text_buffer_changed(&self) {
        let imp = self.imp();
        let buffer = imp.text_view.buffer();
        let text: String = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .into();

        if text.is_empty() {
            imp.formatted_text.replace(None);
            imp.placeholder.set_visible(true);
        } else {
            let formatted_text = FormattedText {
                text,
                entities: vec![],
            };
            imp.formatted_text
                .replace(Some(BoxedFormattedText(formatted_text)));

            imp.placeholder.set_visible(false);
        }

        self.notify("formatted-text");
    }

    /// Insert text inside the message entry at the cursor position,
    /// deleting eventual selected text
    pub(crate) fn insert_at_cursor(&self, text: &str) {
        let buffer = self.imp().text_view.buffer();
        buffer.begin_user_action();
        buffer.delete_selection(true, true);
        buffer.insert_at_cursor(text);
        buffer.end_user_action();
    }

    pub(crate) fn formatted_text(&self) -> Option<BoxedFormattedText> {
        self.imp().formatted_text.borrow().clone()
    }

    pub(crate) fn set_formatted_text(&self, formatted_text: Option<BoxedFormattedText>) {
        if self.formatted_text() == formatted_text {
            return;
        }

        let text = formatted_text.map(|f| f.0.text).unwrap_or_default();
        self.imp().text_view.buffer().set_text(&text);
    }

    pub(crate) async fn as_markdown(&self) -> Option<FormattedText> {
        let text = self.imp().formatted_text.borrow().clone().map(|f| f.0)?;
        let client_id = self.chat().unwrap().session().client_id();

        functions::parse_markdown(text.clone(), client_id)
            .await
            .map(|text| {
                let EnumFormattedText::FormattedText(text) = text;
                text
            })
            .ok()
            .or(Some(text))
    }

    pub(crate) fn chat(&self) -> Option<Chat> {
        self.imp().chat.upgrade()
    }

    pub(crate) fn set_chat(&self, chat: Option<Chat>) {
        if self.chat() == chat {
            return;
        }

        self.imp().chat.set(chat.as_ref());
        self.notify("chat");
    }

    pub(crate) fn connect_formatted_text_notify<F: Fn(&Self, &glib::ParamSpec) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_notify_local(Some("formatted-text"), f)
    }

    pub(crate) fn placeholder_text(&self) -> Option<glib::GString> {
        self.imp().placeholder.text()
    }

    pub(crate) fn set_placeholder_text(&self, placeholder_text: Option<&str>) {
        self.imp().placeholder.set_text(placeholder_text);
    }

    pub(crate) fn connect_activate<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("activate", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
    }

    pub(crate) fn connect_paste_clipboard<F: Fn(&Self) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("paste-clipboard", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
    }

    pub(crate) fn connect_emoji_button_press<F: Fn(&Self, gtk::Image) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("emoji-button-press", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let button = values[1].get::<gtk::Image>().unwrap();
            f(&obj, button);

            None
        })
    }
}

impl Default for MessageEntry {
    fn default() -> Self {
        Self::new()
    }
}
