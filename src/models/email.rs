use askama::Template;

#[derive(Template, Debug)]
#[template(path = "emails/email_verification.html")]
pub struct EmailVerificationCode<'a> {
    pub code: &'a str,
    pub ttl: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "emails/password_verification_code.html")]
pub struct PasswordVerificationCode<'a> {
    pub code: &'a str,
    pub ttl: &'a str,
    pub action: &'a str,
}
