//! Credentials stay in the operating-system Keychain. No command-line or file fallback.
pub struct Secret(Vec<u8>);
impl Secret {
    pub fn new(text: &str) -> Result<Self, &'static str> {
        if !(20..=512).contains(&text.len())
            || !text.starts_with("sk-")
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Use a valid OpenAI API key.");
        }
        Ok(Self(text.as_bytes().to_vec()))
    }
    pub fn from_owned(text: String) -> Result<Self, &'static str> {
        let secret = Self(text.into_bytes());
        let value = secret.text()?;
        if !(20..=512).contains(&value.len())
            || !value.starts_with("sk-")
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Use a valid OpenAI API key.");
        }
        Ok(secret)
    }
    pub fn text(&self) -> Result<&str, &'static str> {
        std::str::from_utf8(&self.0).map_err(|_| "Keychain credential unavailable.")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        for b in &mut self.0 {
            // Best-effort clearing; OS/framework copies have their own lifetimes.
            unsafe { std::ptr::write_volatile(b, 0) }
        }
    }
}
pub trait CredentialStore: Send + Sync {
    fn configured(&self) -> Result<bool, &'static str>;
    fn read(&self) -> Result<Secret, &'static str>;
    fn set(&self, key: &Secret) -> Result<(), &'static str>;
    fn delete(&self) -> Result<(), &'static str>;
}
pub struct Keychain {
    service: String,
}
impl Default for Keychain {
    fn default() -> Self {
        Self {
            service: "app.mochi.openai.v1".into(),
        }
    }
}
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use core_foundation::base::CFTypeRef;
    use core_foundation::string::CFStringRef;
    use core_foundation::{
        base::{CFType, TCFType},
        boolean::CFBoolean,
        data::CFData,
        dictionary::CFDictionary,
        string::CFString,
    };
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecClass: CFStringRef;
        static kSecClassGenericPassword: CFStringRef;
        static kSecAttrService: CFStringRef;
        static kSecAttrAccount: CFStringRef;
        static kSecValueData: CFStringRef;
        static kSecReturnData: CFStringRef;
        static kSecReturnAttributes: CFStringRef;
        fn SecItemCopyMatching(
            query: core_foundation::dictionary::CFDictionaryRef,
            result: *mut CFTypeRef,
        ) -> i32;
        fn SecItemAdd(
            attributes: core_foundation::dictionary::CFDictionaryRef,
            result: *mut CFTypeRef,
        ) -> i32;
        fn SecItemUpdate(
            query: core_foundation::dictionary::CFDictionaryRef,
            attributes: core_foundation::dictionary::CFDictionaryRef,
        ) -> i32;
        fn SecItemDelete(query: core_foundation::dictionary::CFDictionaryRef) -> i32;
    }
    fn key(symbol: CFStringRef) -> CFString {
        // All symbols are framework-owned immutable CFStrings.
        unsafe { CFString::wrap_under_get_rule(symbol) }
    }
    fn query(service: &str) -> Vec<(CFString, CFType)> {
        unsafe {
            vec![
                (key(kSecClass), key(kSecClassGenericPassword).as_CFType()),
                (key(kSecAttrService), CFString::new(service).as_CFType()),
                (key(kSecAttrAccount), CFString::new("personal").as_CFType()),
            ]
        }
    }
    impl CredentialStore for Keychain {
        fn configured(&self) -> Result<bool, &'static str> {
            let mut fields = query(&self.service);
            unsafe {
                fields.push((
                    key(kSecReturnAttributes),
                    CFBoolean::true_value().as_CFType(),
                ));
            }
            let dict = CFDictionary::from_CFType_pairs(&fields);
            let mut value = std::ptr::null();
            let status = unsafe { SecItemCopyMatching(dict.as_concrete_TypeRef(), &mut value) };
            if !value.is_null() {
                drop(unsafe { CFType::wrap_under_create_rule(value) });
            }
            match status {
                0 => Ok(true),
                -25300 => Ok(false),
                _ => Err("Keychain is unavailable or access was denied."),
            }
        }
        fn read(&self) -> Result<Secret, &'static str> {
            let mut fields = query(&self.service);
            unsafe {
                fields.push((key(kSecReturnData), CFBoolean::true_value().as_CFType()));
            }
            let dict = CFDictionary::from_CFType_pairs(&fields);
            let mut value = std::ptr::null();
            let status = unsafe { SecItemCopyMatching(dict.as_concrete_TypeRef(), &mut value) };
            if status != 0 || value.is_null() {
                return Err("Keychain credential unavailable or access denied.");
            }
            let value = unsafe { CFType::wrap_under_create_rule(value) };
            let data = value
                .downcast::<CFData>()
                .ok_or("Keychain credential has an invalid type.")?;
            let text = std::str::from_utf8(data.bytes())
                .map_err(|_| "Keychain credential unavailable.")?;
            Secret::new(text)
        }
        fn set(&self, key_value: &Secret) -> Result<(), &'static str> {
            let fields = query(&self.service);
            let dict = CFDictionary::from_CFType_pairs(&fields);
            let value = unsafe {
                (
                    key(kSecValueData),
                    CFData::from_buffer(key_value.text()?.as_bytes()).as_CFType(),
                )
            };
            let attrs = CFDictionary::from_CFType_pairs(std::slice::from_ref(&value));
            let status =
                unsafe { SecItemUpdate(dict.as_concrete_TypeRef(), attrs.as_concrete_TypeRef()) };
            let status = if status == -25300 {
                let mut fields = fields;
                fields.push(value);
                let attrs = CFDictionary::from_CFType_pairs(&fields);
                unsafe { SecItemAdd(attrs.as_concrete_TypeRef(), std::ptr::null_mut()) }
            } else {
                status
            };
            if status == 0 {
                Ok(())
            } else {
                Err("Keychain could not store the key. No file fallback is used.")
            }
        }
        fn delete(&self) -> Result<(), &'static str> {
            let dict = CFDictionary::from_CFType_pairs(&query(&self.service));
            let status = unsafe { SecItemDelete(dict.as_concrete_TypeRef()) };
            if status == 0 || status == -25300 {
                Ok(())
            } else {
                Err("Keychain could not remove the key.")
            }
        }
    }
}
#[cfg(not(target_os = "macos"))]
impl CredentialStore for Keychain {
    fn configured(&self) -> Result<bool, &'static str> {
        Ok(false)
    }
    fn read(&self) -> Result<Secret, &'static str> {
        Err("macOS Keychain is required.")
    }
    fn set(&self, _: &Secret) -> Result<(), &'static str> {
        Err("macOS Keychain is required.")
    }
    fn delete(&self) -> Result<(), &'static str> {
        Err("macOS Keychain is required.")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_credential_without_disclosing_it() {
        assert!(Secret::new("no-key").is_err());
        assert!(Secret::new("sk-syntheticFixture012345678901234567890").is_ok());
    }
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Requires local native Keychain access; writes and deletes one synthetic disposable entry."]
    fn native_disposable_keychain_roundtrip() {
        let keychain = Keychain {
            service: format!("app.mochi.test.{}", uuid::Uuid::new_v4()),
        };
        let secret =
            Secret::new("sk-syntheticFixture012345678901234567890").expect("synthetic credential");
        struct Cleanup<'a>(&'a Keychain);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete();
            }
        }
        let _cleanup = Cleanup(&keychain);
        assert!(!keychain.configured().expect("keychain available"));
        keychain.set(&secret).expect("native set");
        assert!(keychain.configured().expect("configured"));
        assert!(
            keychain.read().expect("native read").text().expect("valid")
                == secret.text().expect("valid")
        );
        keychain.delete().expect("native delete");
        assert!(!keychain.configured().expect("not configured"));
    }
}
