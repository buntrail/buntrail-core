use buntrail_core::contracts::TargetPath;

#[test]
fn windows_paths_preserve_absolute_native_units() {
    for path in [
        r"C:\work\你好.txt",
        r"c:/work/a.txt",
        r"\\server\share\file",
        r"\\?\C:\work\file",
        r"\\?\UNC\server\share\file",
    ] {
        let units: Vec<u16> = path.encode_utf16().collect();
        assert_eq!(
            TargetPath::windows(units.clone()).unwrap(),
            TargetPath::WindowsWide(units)
        );
    }
    let units = vec![67, 58, 92, 0xd800];
    assert_eq!(
        TargetPath::windows(units.clone()).unwrap(),
        TargetPath::WindowsWide(units)
    );
}

#[test]
fn windows_paths_reject_relative_and_device_namespaces() {
    for path in [
        "",
        r"C:relative",
        r"\rooted",
        r"relative",
        r"\\server",
        r"\\server\",
        r"\\.\pipe\name",
        r"\??\C:\file",
        r"\\?\GLOBALROOT\Device\file",
        r"\\?\C:relative",
        r"\\?\UNC\server",
        r"\\?\C:/file",
    ] {
        assert!(
            TargetPath::windows(path.encode_utf16().collect()).is_err(),
            "accepted {path:?}"
        );
    }
    assert!(TargetPath::windows(vec![67, 58, 92, 0]).is_err());
}

#[test]
fn windows_filesystem_paths_reject_dos_devices_and_unc_ipc() {
    for path in [
        r"C:\NUL",
        r"C:\dir\nul.txt",
        r"C:\NUL. ",
        r"C:\NUL:",
        r"C:\CON",
        r"C:\PRN.txt",
        r"C:\AUX",
        r"C:\COM1",
        r"C:\LPT9",
        r"C:\COM¹",
        r"C:\LPT².log",
        r"C:\COM³",
        r"C:\CONIN$",
        r"C:\CONOUT$",
        r"\\server\pipe\name",
        r"\\server\PiPe\name",
        r"\\server\IPC$\name",
        r"\\?\UNC\server\pipe\name",
        r"\\?\UNC\server\IPC$\name",
    ] {
        assert!(
            TargetPath::windows(path.encode_utf16().collect()).is_err(),
            "accepted device path {path:?}"
        );
    }
    for path in [
        r"C:\COM10.txt",
        r"C:\file.nul",
        r"\\?\C:\NUL",
        r"\\?\UNC\server\share\NUL",
    ] {
        assert!(
            TargetPath::windows(path.encode_utf16().collect()).is_ok(),
            "rejected filesystem path {path:?}"
        );
    }
}

#[test]
fn windows_filesystem_paths_reject_unc_pipe_shares() {
    assert!(TargetPath::windows(r"\\server\pipe\name".encode_utf16().collect()).is_err());
}

#[test]
fn unc_server_and_share_names_are_not_dos_file_components() {
    for path in [r"\\COM1\share\file", r"\\server\NUL\file"] {
        assert!(TargetPath::windows(path.encode_utf16().collect()).is_ok());
    }
}
