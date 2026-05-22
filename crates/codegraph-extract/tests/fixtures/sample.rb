class UserService
  def greet(name)
    format_greeting(name)
  end

  def format_greeting(s)
    "Hi #{s}"
  end
end
