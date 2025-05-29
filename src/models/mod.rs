use askama::Template;

#[derive(Template, Debug)]
#[template(path = "emails/email_verification.html")]
pub struct EmailVerificationCode<'a> {
    pub code: &'a str,
    pub ttl: &'a str,
}
