package com.example;

import java.util.List;

public class UserService {
    public String greet(String name) {
        return formatGreeting(name);
    }

    private String formatGreeting(String s) {
        return "Hi " + s;
    }
}
