# GPUI Community Edition desktop extensions

The seven `gpui*` directories retain the published Apache-2.0 GPUI CE **0.3.0** sources, selected through the root Cargo patch table. They add three small capabilities that are absent from the published API:

- A clockwise `rotation` delta on `PinchEvent`, alongside magnification. Wayland forwards the protocol's rotation delta; XI2 converts fixed-point angle deltas. AppKit's `rotateWithEvent:` is forwarded through the same event. Windows enables and decodes `WM_GESTURE / GID_ROTATE`. Linux scale updates also use relative ratios, matching the PinchEvent contract.
- `Window::paint_image_rotated`, backed by a rotation field on the shared polychrome sprite. The shared Rust shader rotates texture vertices on the GPU. Scene bounds expand to the rotated rectangle. Existing image calls supply zero rotation, preserving the normal renderer path. Metal, Direct3D and WGPU use the same shader definition; source map images remain decoded/uploaded once.
- `Div::pointer_events_none`, backed by `Window::without_hitboxes`. It paints an element and its whole subtree without registering hitboxes, so a surface that is fading out stays visible but no longer swallows clicks meant for the controls behind it. Only prepaint is affected; painting, layout and the fade animation are unchanged.

Windows Precision Touchpad drivers do not universally expose a twist gesture through the OS. The compass cycles N/E/S/W on every desktop; Alt+scroll also rotates. Native gesture input is used when the OS reports it. Physical device support must be verified on the target hardware.

Upstream sources: [GPUI CE](https://github.com/gpui-ce/gpui-ce), [Wayland pointer gestures](https://wayland.app/protocols/pointer-gestures-unstable-v1), [Microsoft WM_GESTURE](https://learn.microsoft.com/en-us/windows/win32/wintouch/wm-gesture), [gesture configuration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-gestureconfig). The native smoke driver also dispatches pinch/rotation through the live GPUI window event path; it is not a claim of physical trackpad validation.
