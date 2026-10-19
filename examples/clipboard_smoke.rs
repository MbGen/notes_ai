use slint::{ComponentHandle, platform::{Platform, WindowAdapter, Clipboard, WindowEvent, PointerEventButton, Key, software_renderer::{MinimalSoftwareWindow,RepaintBufferType}}};
use std::{cell::RefCell,rc::Rc};
slint::slint! {
 import { SelectableNoteText, NotePointer } from "ui/classes-page.slint";
 export component ClipboardFixture inherits Window {
  width: 400px; height: 160px;
  note := SelectableNoteText { x:20px; y:20px; width:360px; height:100px; text:"Alpha beta gamma delta. A complete note."; }
  callback release-copy(bool);
  release-copy(click) => { NotePointer.copy-click=click; NotePointer.released+=1; }
 }
}
struct Backend { window:Rc<MinimalSoftwareWindow>, clipboard:Rc<RefCell<String>> }
impl Platform for Backend {
 fn create_window_adapter(&self)->Result<Rc<dyn WindowAdapter>,slint::PlatformError>{Ok(self.window.clone())}
 fn set_clipboard_text(&self,text:&str,which:Clipboard){if which==Clipboard::DefaultClipboard{*self.clipboard.borrow_mut()=text.into();}}
 fn clipboard_text(&self,_:Clipboard)->Option<String>{Some(self.clipboard.borrow().clone())}
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let window=MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
 let clipboard=Rc::new(RefCell::new(String::new()));
 slint::platform::set_platform(Box::new(Backend{window:window.clone(),clipboard:clipboard.clone()}))?;
 let ui=ClipboardFixture::new()?;ui.show()?;ui.window().set_size(slint::PhysicalSize::new(400,160));
 let mut pixels=slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(400,160);
 window.draw_if_needed(|r|{r.render(pixels.make_mut_slice(),400);});
 let at=|x|slint::LogicalPosition::new(x,28.0);
 ui.window().dispatch_event(WindowEvent::PointerPressed{position:at(25.0),button:PointerEventButton::Left});
 ui.invoke_release_copy(true);slint::platform::update_timers_and_animations();
 ui.window().dispatch_event(WindowEvent::PointerReleased{position:at(25.0),button:PointerEventButton::Left});
 assert_eq!(&*clipboard.borrow(),"Alpha beta gamma delta. A complete note.","single click must copy full note");
 *clipboard.borrow_mut()="sentinel".into();
 ui.window().dispatch_event(WindowEvent::PointerPressed{position:at(25.0),button:PointerEventButton::Left});
 ui.window().dispatch_event(WindowEvent::PointerMoved{position:at(95.0)});
 ui.invoke_release_copy(false);slint::platform::update_timers_and_animations();
 ui.window().dispatch_event(WindowEvent::PointerReleased{position:at(95.0),button:PointerEventButton::Left});
 assert_eq!(&*clipboard.borrow(),"sentinel","drag must not copy entire note");
 ui.window().dispatch_event(WindowEvent::KeyPressed{text:Key::Control.into()});
 ui.window().dispatch_event(WindowEvent::KeyPressed{text:"c".into()});
 ui.window().dispatch_event(WindowEvent::KeyReleased{text:"c".into()});
 ui.window().dispatch_event(WindowEvent::KeyReleased{text:Key::Control.into()});
 let selected=clipboard.borrow().clone();
 assert!(!selected.is_empty()&&selected!="sentinel"&&selected.len()<"Alpha beta gamma delta. A complete note.".len(),"drag selection and Ctrl+C failed: {selected:?}");
 println!("Click copies full note; drag selection and Ctrl+C copy a substring: {selected:?}");Ok(())
}
