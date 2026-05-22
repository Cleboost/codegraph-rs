package main

import "strings"

func ProcessUser(id string) string {
    return formatEmail(id)
}

func formatEmail(s string) string {
    return strings.ToLower(s)
}

type UserService struct {
    Name string
}

func (u *UserService) Greet() string {
    return ProcessUser(u.Name)
}
