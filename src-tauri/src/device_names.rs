/*
  Device names that a user can tell apart.

  Windows names an input device after its driver class, not after the product: every
  keyboard collection comes back as "HID Keyboard Device" and every mouse as
  "HID-compliant mouse". A wireless receiver exposes one collection per function, so the
  Devices tab ended up listing several identical rows with nothing to choose between.

  The specific name is one or two levels up the device tree, on the physical device rather
  than on the HID collection, and the device often reports a product string of its own.
  This module tries both, in that order, and keeps the class name only as a last resort.

  Linux and macOS already report product names, through evdev and IOKit, so there the
  reported name is passed through untouched.
*/

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::{LazyLock, Mutex};

#[cfg(target_os = "windows")]
use windows::Win32;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

#[derive(Clone, Copy)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
}

impl DeviceKind {
    fn fallback_name(self) -> &'static str {
        match self {
            DeviceKind::Keyboard => "Keyboard",
            DeviceKind::Mouse => "Mouse",
        }
    }
}

/// Returns the most specific name the system knows for a device path.
///
/// `reported_name` is the name the grabber library returned, used when nothing better
/// is available.
pub fn friendly_device_name(device_path: &str, reported_name: &str, kind: DeviceKind) -> String {
    #[cfg(target_os = "windows")] {
        // Discovery runs every second, and a name never changes for a given path.
        if let Ok(resolved_names) = RESOLVED_NAMES.lock() {
            if let Some(name) = resolved_names.get(device_path) {
                return name.clone(); // Early return
            }
        }

        let name = resolve_name(device_path, reported_name, kind);
        if let Ok(mut resolved_names) = RESOLVED_NAMES.lock() {
            resolved_names.insert(device_path.to_string(), name.clone());
        }
        name
    }

    #[cfg(not(target_os = "windows"))] {
        let _ = device_path;
        let reported_name = reported_name.trim();
        if reported_name.is_empty() {
            kind.fallback_name().to_string()
        } else {
            reported_name.to_string()
        }
    }
}

/*
  Names that describe a driver class or a bus instead of a device. A name containing one
  of these tells the user nothing, so the search carries on up the device tree.
  Matched in lower case, on a substring, because Windows pads them ("USB Root Hub (USB 3.0)").
*/
#[cfg(target_os = "windows")]
const GENERIC_NAME_PARTS: [&str; 18] = [
    "hid keyboard device",
    "hid-compliant mouse",
    "hid compliant mouse",
    "hid-compliant device",
    "usb input device",
    "usb composite device",
    "usb human interface device",
    "usb keyboard",
    "usb mouse",
    "bluetooth low energy gatt compliant hid device",
    "bluetooth hid device",
    "bluetooth le device",
    "bluetooth le service",
    "unknown keyboard name",
    "unknown mouse name",
    "unnamed device",
    "root hub",
    "host controller",
];

/// How far up the device tree to look. The physical device sits one level above a USB
/// collection and two above a Bluetooth one; past that are hubs and controllers.
#[cfg(target_os = "windows")]
const PARENT_LEVELS_SEARCHED: u32 = 3;

#[cfg(target_os = "windows")]
static RESOLVED_NAMES: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(target_os = "windows")]
fn resolve_name(device_path: &str, reported_name: &str, kind: DeviceKind) -> String {
    // What the device calls itself, which is the best name when the device provides it.
    if let Some(name) = hid_product_string(device_path) {
        if is_specific(&name) {
            return name; // Early return
        }
    }

    // Otherwise the closest name to the device that is not a driver class or a bus.
    if let Some(name) = device_tree_name(device_path) {
        return name; // Early return
    }

    let reported_name = reported_name.trim();
    if reported_name.is_empty() {
        kind.fallback_name().to_string()
    } else {
        reported_name.to_string()
    }
}

#[cfg(target_os = "windows")]
fn is_specific(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    !name.is_empty() && !GENERIC_NAME_PARTS.iter().any(|generic_part| name.contains(generic_part))
}

#[cfg(target_os = "windows")]
fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|character| *character == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end]).trim().to_string()
}

/// Reads the product string the device reports over HID, when it reports one.
#[cfg(target_os = "windows")]
fn hid_product_string(device_path: &str) -> Option<String> {
    let wide_device_path = to_wide(device_path);
    unsafe {
        // No access rights are requested: HidD only needs the handle, and a keyboard
        // would refuse to open for reading.
        let handle = Win32::Storage::FileSystem::CreateFileW(
            PCWSTR(wide_device_path.as_ptr()),
            0,
            Win32::Storage::FileSystem::FILE_SHARE_READ | Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            Win32::Storage::FileSystem::OPEN_EXISTING,
            Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        ).ok()?;

        let mut buffer = [0u16; 128];
        let read = Win32::Devices::HumanInterfaceDevice::HidD_GetProductString(
            handle,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            (buffer.len() * size_of::<u16>()) as u32,
        );
        let _ = Win32::Foundation::CloseHandle(handle);
        if !read {
            return None; // Early return
        }

        let product_string = from_wide(&buffer);
        if product_string.is_empty() { None } else { Some(product_string) }
    }
}

/// A device path such as "\\?\HID#VID_1D57&PID_FA60&MI_00#8&106ef33&0&0000#{884b96c3-...}"
/// holds the device instance id "HID\VID_1D57&PID_FA60&MI_00\8&106ef33&0&0000".
#[cfg(target_os = "windows")]
fn instance_id_from_path(device_path: &str) -> String {
    let path = device_path.trim_start_matches(r"\\?\").trim_start_matches(r"\\.\");
    let without_interface_guid = match path.rfind("#{") {
        Some(index) => &path[..index],
        None => path,
    };
    without_interface_guid.replace('#', r"\")
}

/// Walks from the device towards the machine, returning the first name that describes a
/// device rather than a driver class or a bus.
#[cfg(target_os = "windows")]
fn device_tree_name(device_path: &str) -> Option<String> {
    let instance_id = to_wide(&instance_id_from_path(device_path));
    let mut device_node: u32 = 0;
    unsafe {
        let located = Win32::Devices::DeviceAndDriverInstallation::CM_Locate_DevNodeW(
            &mut device_node,
            PCWSTR(instance_id.as_ptr()),
            Win32::Devices::DeviceAndDriverInstallation::CM_LOCATE_DEVNODE_NORMAL,
        );
        if located != Win32::Devices::DeviceAndDriverInstallation::CR_SUCCESS {
            return None; // Early return
        }
    }

    for level in 0..=PARENT_LEVELS_SEARCHED {
        let names = [
            // The name the user gave the device, then the name the device gave the bus,
            // then the name the driver class gave it.
            device_node_property(device_node, &Win32::Devices::Properties::DEVPKEY_Device_FriendlyName),
            device_node_property(device_node, &Win32::Devices::Properties::DEVPKEY_Device_BusReportedDeviceDesc),
            device_node_property(device_node, &Win32::Devices::Properties::DEVPKEY_NAME),
        ];
        for name in names.into_iter().flatten() {
            if is_specific(&name) {
                return Some(name); // Early return
            }
        }

        if level == PARENT_LEVELS_SEARCHED {
            break;
        }
        let mut parent_device_node: u32 = 0;
        unsafe {
            let found_parent = Win32::Devices::DeviceAndDriverInstallation::CM_Get_Parent(
                &mut parent_device_node,
                device_node,
                0,
            );
            if found_parent != Win32::Devices::DeviceAndDriverInstallation::CR_SUCCESS {
                break;
            }
        }
        device_node = parent_device_node;
    }

    None
}

#[cfg(target_os = "windows")]
fn device_node_property(device_node: u32, property_key: &Win32::Foundation::DEVPROPKEY) -> Option<String> {
    let mut property_type = Win32::Devices::Properties::DEVPROPTYPE::default();
    let mut size: u32 = 0;
    unsafe {
        // First call to get the required buffer size
        let _ = Win32::Devices::DeviceAndDriverInstallation::CM_Get_DevNode_PropertyW(
            device_node,
            property_key,
            &mut property_type,
            None,
            &mut size,
            0,
        );
        if size == 0 {
            return None; // Early return, the device node does not have this property
        }

        let mut buffer: Vec<u8> = vec![0; size as usize];
        let read = Win32::Devices::DeviceAndDriverInstallation::CM_Get_DevNode_PropertyW(
            device_node,
            property_key,
            &mut property_type,
            Some(buffer.as_mut_ptr()),
            &mut size,
            0,
        );
        if read != Win32::Devices::DeviceAndDriverInstallation::CR_SUCCESS {
            return None; // Early return
        }
        buffer.truncate(size as usize);

        let wide_buffer: Vec<u16> = buffer.chunks_exact(2)
            .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
            .collect();
        let property = from_wide(&wide_buffer);
        if property.is_empty() { None } else { Some(property) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reported_name_is_used_when_nothing_better_exists() {
        // A path that no system can resolve, so the reported name has to come through.
        let name = friendly_device_name("not-a-device-path/reported", "Some Keyboard", DeviceKind::Keyboard);
        assert_eq!(name, "Some Keyboard");
    }

    #[test]
    fn an_empty_reported_name_falls_back_to_the_device_kind() {
        // A path of its own per case, because a resolved name is cached against the path.
        assert_eq!(friendly_device_name("not-a-device-path/keyboard", "", DeviceKind::Keyboard), "Keyboard");
        assert_eq!(friendly_device_name("not-a-device-path/mouse", "  ", DeviceKind::Mouse), "Mouse");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_device_path_holds_an_instance_id() {
        assert_eq!(
            instance_id_from_path(r"\\?\HID#VID_1D57&PID_FA60&MI_00#8&106ef33&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}"),
            r"HID\VID_1D57&PID_FA60&MI_00\8&106ef33&0&0000"
        );
        assert_eq!(
            instance_id_from_path(r"\\?\ROOT#FEIZHI_VIRTUAL_KEYBOARD#0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}"),
            r"ROOT\FEIZHI_VIRTUAL_KEYBOARD\0000"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn driver_class_names_are_not_specific() {
        assert!(!is_specific("HID Keyboard Device"));
        assert!(!is_specific("HID-compliant mouse"));
        assert!(!is_specific("USB Input Device"));
        assert!(!is_specific("Bluetooth LE Device d417a7dbbcfe"));
        assert!(!is_specific(""));
        assert!(is_specific("2.4G Wireless Device"));
        assert!(is_specific("M87-BT1"));
    }
}
