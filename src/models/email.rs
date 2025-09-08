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

#[derive(Template, Debug)]
#[template(path = "emails/withdraw_paypal.html")]
pub struct WithdrawPaypal<'a> {
    pub amount: f64,
    pub paypal_email: &'a str,
    pub support_email: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "emails/paypal_unclaimed.html")]
pub struct PaypalUnclaimed<'a> {
    pub amount: &'a str,
    pub paypal_email: &'a str,
}
