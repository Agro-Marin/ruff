use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_test_path};
use crate::rules::odoo::model::{field_declarations, field_type};

/// ## What it does
/// Checks for a secret (a password, an API key, a token, a private key...)
/// kept in a plain stored `Char` or `Text` column, in a settings field backed
/// by `config_parameter`, or read or written as an `ir.config_parameter`.
///
/// ## Why is this bad?
/// A plain column or an `ir.config_parameter` is in every backup in clear. The
/// secret belongs in `credential.credential`, held by a Many2one, with a
/// computed field of the old name reading it through the vault's use path.
///
/// A name about a secret rather than one (`token_type`, `has_api_key`,
/// `api_key_url`...), a capability minted for a link, a derived value (a hash,
/// a masked or sealed copy), a feed cursor, a key published on purpose and a
/// value the module hashes are not reported, nor the fields and parameters
/// judged one by one not to be secrets.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct CredentialStorage {
    what: String,
}

impl Violation for CredentialStorage {
    #[derive_message_formats]
    fn message(&self) -> String {
        let CredentialStorage { what } = self;
        format!("{what}, in clear")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Keep the secret in `credential.credential` and hold a Many2one to it".to_string())
    }
}

/// The vault holds its own values; it cannot reference itself.
const VAULT_MODULE: &str = "credential";

static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(password|secret|api_?key|token|passphrase|private_key|client_secret|credential|_key$)",
    )
    .expect("valid regex")
});
/// Handed to browsers on purpose; vaulting one protects nothing.
static PUBLIC_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(public_key|publishable_key|site_key|website_key|client_key)$")
        .expect("valid regex")
});
/// An index, a lookup, a keyboard key: not a credential in any sense.
static NOT_A_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(cache_key|bucket_key|grouping_key|job_key|period_key|source_key|zip_key|website_form_key|avatar_cache_key|push_to_talk_key|attendance_kiosk_key|identity_key|booking_key)$",
    )
    .expect("valid regex")
});
/// Names that are about a secret rather than one.
static ABOUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(token_type|_expir|has_|is_|use_|show_|_count|_url|_uri|_header|_name|_id$|_ids$|_state|_status|_method|_type$|_scheme$|_endpoint$)",
    )
    .expect("valid regex")
});
/// A capability we mint so a link works; moving it into the vault breaks the URL.
static SHARE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(share_token|invite_token|document_token|portal_token|signup_token)$")
        .expect("valid regex")
});
/// Computed from a secret, and the point of them is that they are not it; a
/// sealed value is encrypted under a key the database does not hold in clear.
static DERIVED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(_hash|_masked|_fingerprint|_encrypted|_sealed|_plain|_display)$")
        .expect("valid regex")
});
/// A cursor the counterparty hands back so the next call resumes a feed.
static CURSOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(sync_token|page_token|next_token|_cursor)$").expect("valid regex")
});

/// Decided per field, because the name does not say: identifiers printed on a
/// document or sent in the clear, and tokens that authorise a visitor to one of
/// our records rather than us to somebody's API, or that we publish on purpose;
/// an access.link's nonce is useless without the sealed database secret, and
/// its hint is four characters of the token.
const JUDGED_NOT_SECRET: &[&str] = &[
    "account.counterparty_key",
    "appointment_account_payment.booking_token",
    "appointment_google_reserve.google_reserve_access_token",
    "appointment_google_reserve.google_reserve_idempotency_token",
    "approval.subject_key",
    "trade.tax_ids_auto_key",
    "conversation.conversation_key",
    "device_gps.gps_receiver_key",
    "device_gps_geoengine.current_address_key",
    "auth_passkey.credential_identifier",
    "delivery_fedex.fedex_developer_key",
    "delivery_fedex_rest.fedex_rest_developer_key",
    "delivery_usps_rest.usps_extra_data_payment_token_request",
    "sale_lazada.app_key",
    "l10n_br_edi.l10n_br_access_key",
    "l10n_br_edi_pos.l10n_br_access_key",
    "sale_amazon.seller_key",
    "website.google_analytics_key",
    "website_slides.website_slide_google_app_key",
    "equity.equity_access_token",
    "hr_contract_salary.hash_token",
    "mail_mobile.ocn_token",
    "planning.employee_token",
    "planning.planning_token",
    "web_map.map_box_token",
    "base.access_token",
    "base.token_hint",
    "base.token_nonce",
    "calendar.access_token",
    "calendar.booking_access_token",
    "document.access_token",
    "frontdesk.access_token",
    "hr_contract_salary.access_token",
    "iot.token",
    "mail.access_token",
    "planning.access_token",
    "point_of_sale.access_token",
    "portal.access_token",
    "pos_enterprise.access_token",
    "rating.access_token",
    "room.access_token",
    "sign.access_token",
    "social_instagram.instagram_access_token",
    "social_push_notifications.firebase_push_certificate_key",
    "social_push_notifications.firebase_web_api_key",
    "sign.sms_token",
    "sign.token",
    "social_push_notifications.push_token",
    "spreadsheet_dashboard.access_token",
    "survey.access_token",
    "website.access_token",
    "website_sale.access_token",
    "website_slides.access_token",
    "website.google_maps_api_key",
];

/// Parameter keys whose name says secret but whose value is not one kept in
/// clear: switches and lifetimes, a publishable map token, a one-time pairing
/// token that expires in minutes, a demo-mode sentinel, and the database's HMAC
/// seed, which `ir.config_parameter` itself seals at rest.
const JUDGED_PARAMETERS_NOT_SECRET: &[&str] = &[
    "auth_signup.reset_password",
    "database.secret",
    "hr_contract_salary.access_token_validity",
    "iot.iot_token",
    "mail.chat_from_token",
    "portal.allow_api_keys",
    "pos_tyro.api_key",
    "pos_urban_piper.urbanpiper_apikey",
    "social.twitter_consumer_key",
    "web_map.token_map_box",
];

/// The module of a path: the directory after its last `/addons/`.
fn module_of(path: &str) -> Option<&str> {
    let (_, tail) = path.rsplit_once("/addons/")?;
    Some(tail.split('/').next().unwrap_or(tail))
}

fn looks_secret(module: &str, name: &str, hashed: &FxHashSet<&str>) -> bool {
    SECRET.is_match(name)
        && !ABOUT.is_match(name)
        && !SHARE.is_match(name)
        && !DERIVED.is_match(name)
        && !CURSOR.is_match(name)
        && !PUBLIC_KEY.is_match(name)
        && !NOT_A_KEY.is_match(name)
        && !hashed.contains(name)
        && !JUDGED_NOT_SECRET.contains(&format!("{module}.{name}").as_str())
}

/// The keywords of a call as a dict reads them: the last of a repeated name wins.
fn keyword<'a>(call: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    call.arguments
        .keywords
        .iter()
        .rev()
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

fn is_stored(call: &ast::ExprCall) -> bool {
    match keyword(call, "store") {
        Some(Expr::BooleanLiteral(store)) => store.value,
        _ => keyword(call, "compute").is_none() && keyword(call, "related").is_none(),
    }
}

#[derive(Default)]
struct HashCall {
    found: bool,
}

impl<'a> Visitor<'a> for HashCall {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && matches!(&*call.func, Expr::Attribute(attribute) if attribute.attr.as_str() == "hash")
        {
            self.found = true;
        }
        if !self.found {
            visitor::walk_expr(self, expr);
        }
    }
}

/// The names the module assigns a `.hash(...)` result to, the classes of the
/// module, and its parameter reads and writes.
#[derive(Default)]
struct Module<'a> {
    hashed: FxHashSet<&'a str>,
    classes: Vec<&'a ast::StmtClassDef>,
    parameters: Vec<(&'a ast::ExprCall, &'a str)>,
}

impl<'a> Visitor<'a> for Module<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                let mut hash = HashCall::default();
                hash.visit_expr(value);
                if hash.found {
                    self.hashed
                        .extend(targets.iter().filter_map(|target| match target {
                            Expr::Attribute(attribute) => Some(attribute.attr.as_str()),
                            Expr::Name(name) => Some(name.id.as_str()),
                            _ => None,
                        }));
                }
            }
            Stmt::ClassDef(class) => self.classes.push(class),
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && let Expr::Attribute(attribute) = &*call.func
            && matches!(attribute.attr.as_str(), "get_param" | "set_param")
            && let Some(Expr::StringLiteral(key)) = call.arguments.args.first()
        {
            self.parameters.push((call, key.value.to_str()));
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8520
pub(crate) fn credential_storage(checker: &Checker, suite: &Suite) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    let path = path.to_string_lossy();
    let Some(module) =
        module_of(&path).filter(|module| !module.is_empty() && *module != VAULT_MODULE)
    else {
        return;
    };
    let mut nodes = Module::default();
    nodes.visit_body(suite);
    let mut found: Vec<(TextRange, String)> = Vec::new();
    for (call, key) in &nodes.parameters {
        if JUDGED_PARAMETERS_NOT_SECRET.contains(key) {
            continue;
        }
        let name = key.rsplit('.').next().unwrap_or(key);
        if looks_secret("", name, &nodes.hashed) {
            found.push((
                call.range(),
                format!("`{key}` is read or written as an `ir.config_parameter`"),
            ));
        }
    }
    for class in &nodes.classes {
        let transient = class
            .bases()
            .first()
            .is_some_and(|base| checker.locator().slice(base).contains("TransientModel"));
        for (name, call, statement) in field_declarations(class) {
            if !matches!(field_type(call), "Char" | "Text")
                || !looks_secret(module, name, &nodes.hashed)
            {
                continue;
            }
            if transient {
                if keyword(call, "config_parameter").is_some() {
                    found.push((
                        statement.range(),
                        format!("`{module}.{name}` keeps a secret in `ir.config_parameter`"),
                    ));
                }
            } else if is_stored(call) {
                found.push((
                    statement.range(),
                    format!("`{module}.{name}` stores a secret in a plain column"),
                ));
            }
        }
    }
    for (range, what) in found {
        checker.report_diagnostic(CredentialStorage { what }, range);
    }
}
