def on_denied():
    log_error()


def on_ok():
    send_request()


def log_error():
    pass


def send_request():
    pass


def handle(permitted: bool):
    if permitted:
        on_ok()
    else:
        on_denied()
