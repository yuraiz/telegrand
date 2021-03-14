use grammers_client::InputMessage;
use grammers_client::types::{Dialog, Message};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::glib;
use std::sync::Arc;
use tokio::runtime;
use tokio::sync::mpsc;

use crate::message_row::MessageRow;
use crate::telegram;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use std::cell::RefCell;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/chat_page.ui")]
    pub struct ChatPage {
        #[template_child]
        pub messages_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub message_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub send_message_button: TemplateChild<gtk::Button>,
        pub dialog: RefCell<Option<Arc<Dialog>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatPage {
        const NAME: &'static str = "ChatPage";
        type Type = super::ChatPage;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatPage {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
        }
    }

    impl WidgetImpl for ChatPage {}
    impl BoxImpl for ChatPage {}
}

glib::wrapper! {
    pub struct ChatPage(ObjectSubclass<imp::ChatPage>)
        @extends gtk::Widget, gtk::Box;
}

impl ChatPage {
    pub fn new(tg_sender: &mpsc::Sender<telegram::EventTG>, dialog: Dialog) -> Self {
        let chat_page = glib::Object::new(&[])
            .expect("Failed to create ChatPage");

        let self_ = imp::ChatPage::from_instance(&chat_page);
        self_.dialog.replace(Some(Arc::new(dialog)));
        let dialog = self_.dialog.clone();

        let message_entry = &*self_.message_entry;
        let tg_sender_clone = tg_sender.clone();
        self_.send_message_button
            .connect_clicked(glib::clone!(@weak message_entry => move |_| {
                let dialog = &*dialog.borrow();
                let dialog = dialog.as_ref().unwrap().clone();
                let message = InputMessage::text(message_entry.get_text());
                message_entry.set_text("");

                let _ = runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(
                        tg_sender_clone.send(telegram::EventTG::SendMessage(
                            dialog, message)));
            }));

        chat_page
    }

    pub fn update_chat(&self, tg_sender: &mpsc::Sender<telegram::EventTG>) {
        let self_ = imp::ChatPage::from_instance(self);
        let messages_list = &*self_.messages_list;

        if let None = messages_list.get_row_at_y(0) {
            let dialog = &*self_.dialog.borrow();
            let dialog = dialog.as_ref().unwrap().clone();
            let _ = runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    tg_sender.send(telegram::EventTG::RequestMessages(dialog)));
        }
    }

    pub fn add_message(&self, message: Message) {
        let message_row = MessageRow::new(message);

        let self_ = imp::ChatPage::from_instance(self);
        self_.messages_list.prepend(&message_row);
    }
}
