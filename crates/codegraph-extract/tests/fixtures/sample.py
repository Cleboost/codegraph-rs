from typing import List

def process_user(uid: str) -> str:
    """Format and return a user identifier."""
    return format_email(uid)

def format_email(s: str) -> str:
    return s.lower()

class UserService:
    def __init__(self, name: str):
        self.name = name

    def greet(self) -> str:
        return process_user(self.name)
