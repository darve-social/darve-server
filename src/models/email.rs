use askama::Template;

#[derive(Template, Debug)]
#[template(path = "emails/email_verification.html")]
pub struct EmailVerificationCode<'a> {
    pub code: &'a str,
    pub ttl: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "emails/reset_password.html")]
pub struct ResetPassword<'a> {
    pub code: &'a str,
    pub ttl: &'a str,
}
