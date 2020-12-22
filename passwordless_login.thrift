enum PasswordLessLoginReturnCode {
    PLL_SUCCESS = 0,
    PLL_NOT_AUTHORIZED = 1,
    PLL_ERROR = 2
}

struct PasswordLessLoginResult {
  1: PasswordLessLoginReturnCode returnCode,
  2: string username,
  3: string cookieName,
  4: string cookieValue,
  5: i32 cookieMaxAge
}

service PasswordLessLogin {
  PasswordLessLoginResult login()
}
