fn main() {
    app::run();
}

mod app {
    use multitouch::{Contact, ContactStream, Device, Finger};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
        NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSFontWeightMedium,
        NSFontWeightRegular, NSForegroundColorAttributeName, NSMenu, NSMenuItem, NSStringDrawing,
        NSView, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{
        NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSTimer,
    };
    use std::cell::RefCell;

    /// Approximate aspect ratio (width / height) of a MacBook trackpad.
    const TRACKPAD_ASPECT: f64 = 1.6;
    const HEADER_HEIGHT: f64 = 44.0;
    const FOOTER_HEIGHT: f64 = 176.0;
    const MARGIN: f64 = 24.0;
    const MAX_LISTED: usize = 8;

    struct ViewState {
        device: Device,
        stream: ContactStream,
        contacts: RefCell<Vec<Contact>>,
    }

    define_class!(
        #[unsafe(super(NSView))]
        #[thread_kind = MainThreadOnly]
        #[name = "MultitouchVisualizerView"]
        #[ivars = ViewState]
        struct TouchView;

        impl TouchView {
            #[unsafe(method(isFlipped))]
            fn is_flipped(&self) -> bool {
                true
            }

            #[unsafe(method(tick:))]
            fn tick(&self, _timer: &NSTimer) {
                let state = self.ivars();
                let mut latest = None;
                while let Some(frame) = state.stream.try_recv() {
                    latest = Some(frame);
                }
                if let Some(frame) = latest {
                    *state.contacts.borrow_mut() = frame;
                    self.setNeedsDisplay(true);
                }
            }

            #[unsafe(method(drawRect:))]
            fn draw_rect(&self, _dirty: NSRect) {
                let state = self.ivars();
                draw(self.bounds(), &state.device, &state.contacts.borrow());
            }
        }
    );

    impl TouchView {
        fn new(mtm: MainThreadMarker, frame: NSRect, state: ViewState) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(state);
            unsafe { msg_send![super(this), initWithFrame: frame] }
        }
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "MultitouchVisualizerDelegate"]
        struct AppDelegate;

        unsafe impl NSObjectProtocol for AppDelegate {}

        unsafe impl NSApplicationDelegate for AppDelegate {
            #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
            fn should_terminate_after_last_window_closed(&self, _app: &NSApplication) -> bool {
                true
            }
        }
    );

    impl AppDelegate {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send![super(this), init] }
        }
    }

    pub fn run() {
        if !Device::is_available() {
            eprintln!("MultitouchSupport is unavailable on this Mac.");
            return;
        }
        let Some(device) = Device::default() else {
            eprintln!("No multitouch device found.");
            return;
        };
        println!("using {} ({:?})", device.name(), device.device_id());
        if !device.start() {
            eprintln!("Failed to start device.");
            return;
        }
        let stream = device.contact_frames();

        let mtm = MainThreadMarker::new().expect("must run on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let delegate = AppDelegate::new(mtm);
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        install_menu(mtm, &app);

        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(860.0, 720.0));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                content,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str("Multitouch Visualizer"));
        window.setMinSize(NSSize::new(560.0, 520.0));

        let view = TouchView::new(
            mtm,
            content,
            ViewState {
                device,
                stream,
                contacts: RefCell::new(Vec::new()),
            },
        );
        window.setContentView(Some(&view));
        window.center();
        window.makeKeyAndOrderFront(None);

        // Poll the contact stream on the main thread at ~120 Hz.
        let target: &AnyObject = &view;
        let _timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                1.0 / 120.0,
                target,
                sel!(tick:),
                None,
                true,
            )
        };

        app.activate();
        app.run();
    }

    fn install_menu(mtm: MainThreadMarker, app: &NSApplication) {
        let main_menu = NSMenu::new(mtm);
        let app_item = NSMenuItem::new(mtm);
        main_menu.addItem(&app_item);

        let app_menu = NSMenu::new(mtm);
        let quit = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Quit Multitouch Visualizer"),
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        app_menu.addItem(&quit);
        app_item.setSubmenu(Some(&app_menu));
        app.setMainMenu(Some(&main_menu));
    }

    fn finger_color(contact: &Contact) -> Retained<NSColor> {
        if contact.is_palm() {
            return NSColor::systemGrayColor();
        }
        match contact.finger() {
            Some(Finger::Thumb) => NSColor::systemRedColor(),
            Some(Finger::Index) => NSColor::systemOrangeColor(),
            Some(Finger::Middle) => NSColor::systemGreenColor(),
            Some(Finger::Ring) => NSColor::systemBlueColor(),
            Some(Finger::Pinky) => NSColor::systemPurpleColor(),
            None => NSColor::systemGrayColor(),
        }
    }

    fn finger_label(contact: &Contact) -> &'static str {
        if contact.is_palm() {
            return "P";
        }
        match contact.finger() {
            Some(Finger::Thumb) => "1",
            Some(Finger::Index) => "2",
            Some(Finger::Middle) => "3",
            Some(Finger::Ring) => "4",
            Some(Finger::Pinky) => "5",
            None => "•",
        }
    }

    fn draw_text(
        text: &str,
        x: f64,
        y: f64,
        size: f64,
        bold: bool,
        color: &NSColor,
        centered: bool,
    ) {
        let weight = unsafe {
            if bold {
                NSFontWeightMedium
            } else {
                NSFontWeightRegular
            }
        };
        let font = NSFont::monospacedSystemFontOfSize_weight(size, weight);
        let font_obj: &AnyObject = &font;
        let color_obj: &AnyObject = color;
        let attrs = unsafe {
            NSDictionary::from_slices(
                &[NSFontAttributeName, NSForegroundColorAttributeName],
                &[font_obj, color_obj],
            )
        };
        let string = NSString::from_str(text);
        unsafe {
            let mut origin = NSPoint::new(x, y);
            if centered {
                let extent = string.sizeWithAttributes(Some(&attrs));
                origin.x -= extent.width / 2.0;
                origin.y -= extent.height / 2.0;
            }
            string.drawAtPoint_withAttributes(origin, Some(&attrs));
        }
    }

    fn draw(bounds: NSRect, device: &Device, contacts: &[Contact]) {
        let width = bounds.size.width;
        let height = bounds.size.height;

        NSColor::windowBackgroundColor().setFill();
        NSBezierPath::fillRect(bounds);

        let label = NSColor::labelColor();
        let secondary = NSColor::secondaryLabelColor();

        let visible: Vec<&Contact> = contacts
            .iter()
            .filter(|c| {
                !matches!(
                    c.state(),
                    multitouch::ContactState::NotTracking | multitouch::ContactState::OutOfRange
                )
            })
            .collect();

        draw_text(&device.name(), MARGIN, 14.0, 15.0, true, &label, false);
        let summary = format!(
            "{} contact{}",
            visible.len(),
            if visible.len() == 1 { "" } else { "s" }
        );
        draw_text(&summary, MARGIN, 32.0 - 2.0, 11.0, false, &secondary, false);

        // Aspect-fit the trackpad surface between header and footer.
        let avail_w = (width - MARGIN * 2.0).max(1.0);
        let avail_h = (height - HEADER_HEIGHT - FOOTER_HEIGHT - MARGIN).max(1.0);
        let (pad_w, pad_h) = if avail_w / avail_h > TRACKPAD_ASPECT {
            (avail_h * TRACKPAD_ASPECT, avail_h)
        } else {
            (avail_w, avail_w / TRACKPAD_ASPECT)
        };
        let pad = NSRect::new(
            NSPoint::new(
                (width - pad_w) / 2.0,
                HEADER_HEIGHT + (avail_h - pad_h) / 2.0,
            ),
            NSSize::new(pad_w, pad_h),
        );

        let surface = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(pad, 18.0, 18.0);
        NSColor::controlBackgroundColor().setFill();
        surface.fill();
        NSColor::separatorColor().setStroke();
        surface.setLineWidth(1.5);
        surface.stroke();

        for contact in &visible {
            let p = contact.normalized.position;
            let cx = pad.origin.x + p.x.clamp(0.0, 1.0) as f64 * pad.size.width;
            // Multitouch coordinates are y-up; the flipped view is y-down.
            let cy = pad.origin.y + (1.0 - p.y.clamp(0.0, 1.0)) as f64 * pad.size.height;

            let color = finger_color(contact);
            let active = contact.state().is_active();
            let alpha = if active { 1.0 } else { 0.4 };
            let radius =
                (8.0 + contact.major_axis.max(contact.minor_axis) as f64 * 1.2).clamp(14.0, 34.0);

            // Velocity trail.
            let v = contact.normalized.velocity;
            let speed = ((v.x * v.x + v.y * v.y) as f64).sqrt();
            if speed > 0.01 {
                let scale = (pad.size.width * 0.12).min(90.0);
                let tail = NSPoint::new(cx + v.x as f64 * scale, cy - v.y as f64 * scale);
                color.colorWithAlphaComponent(0.6).setStroke();
                let line = NSBezierPath::bezierPath();
                line.setLineWidth(3.0);
                line.moveToPoint(NSPoint::new(cx, cy));
                line.lineToPoint(tail);
                line.stroke();
            }

            // Pressure halo.
            let halo = radius + (contact.pressure as f64 / 40.0).clamp(0.0, 14.0);
            let halo_rect = NSRect::new(
                NSPoint::new(cx - halo, cy - halo),
                NSSize::new(halo * 2.0, halo * 2.0),
            );
            color.colorWithAlphaComponent(0.18 * alpha).setFill();
            NSBezierPath::bezierPathWithOvalInRect(halo_rect).fill();

            let rect = NSRect::new(
                NSPoint::new(cx - radius, cy - radius),
                NSSize::new(radius * 2.0, radius * 2.0),
            );
            let dot = NSBezierPath::bezierPathWithOvalInRect(rect);
            color.colorWithAlphaComponent(0.85 * alpha).setFill();
            dot.fill();
            color.colorWithAlphaComponent(alpha).setStroke();
            dot.setLineWidth(2.0);
            dot.stroke();

            draw_text(
                finger_label(contact),
                cx,
                cy,
                13.0,
                true,
                &NSColor::whiteColor(),
                true,
            );
        }

        let mut y = pad.origin.y + pad.size.height + 16.0;
        draw_text(
            "1–5 = classified finger   P = palm   line = velocity   halo = pressure",
            MARGIN,
            y,
            11.0,
            false,
            &secondary,
            false,
        );
        y += 20.0;
        for contact in visible.iter().take(MAX_LISTED) {
            let finger = contact
                .finger()
                .map(|f| f.to_string())
                .unwrap_or_else(|| "palm".into());
            let line = format!(
                "id {:>2} {:>9} {:>6}  pos({:>5.3},{:>5.3})  vel({:>6.3},{:>6.3})  p {:>6.1}  d {:>5.2}  axes {:>5.2}/{:>5.2}",
                contact.id,
                contact.state(),
                finger,
                contact.normalized.position.x,
                contact.normalized.position.y,
                contact.normalized.velocity.x,
                contact.normalized.velocity.y,
                contact.pressure,
                contact.density,
                contact.major_axis,
                contact.minor_axis,
            );
            draw_text(&line, MARGIN, y, 11.0, false, &label, false);
            y += 15.0;
        }
    }
}
