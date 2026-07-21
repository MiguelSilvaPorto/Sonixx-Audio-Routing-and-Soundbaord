import sys, os, ctypes, traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def is_admin():
    try:
        return ctypes.windll.shell32.IsUserAnAdmin()
    except Exception:
        return False


def main():
    if not is_admin():
        # Restart the script as admin
        ctypes.windll.shell32.ShellExecuteW(None, "runas", sys.executable, " ".join(sys.argv), None, 1)
        sys.exit()

    try:
        from app.ui import App
        app = App()
        app.mainloop()
    except Exception as e:
        err_msg = traceback.format_exc()
        print(f"\n[Sonixx Fatal Error]\n{err_msg}")
        try:
            ctypes.windll.user32.MessageBoxW(0, f"Sonixx encountered a startup error:\n\n{e}", "Sonixx Startup Error", 0x10)
        except Exception:
            pass
        try:
            if sys.stdin and hasattr(sys.stdin, "read"):
                input("Press Enter to exit...")
        except Exception:
            pass


if __name__ == "__main__":
    main()
