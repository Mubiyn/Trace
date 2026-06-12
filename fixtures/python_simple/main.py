from utils import format_message


def greet(name: str) -> str:
    return format_message(name)


if __name__ == "__main__":
    print(greet("world"))
