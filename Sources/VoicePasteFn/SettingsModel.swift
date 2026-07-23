import Foundation
import Security

// MARK: - Settings (UserDefaults + Keychain, hot-reloadable)
//
// Storage layout follows macOS conventions:
//   - non-secret settings → UserDefaults
//     (persisted to ~/Library/Preferences/com.bezrabotnyi.voicepastefn.plist)
//   - API key → Keychain (system-encrypted, only this app can read)
//   - env vars override UserDefaults/Keychain for the current launch
//     (lets a shell-launched `swift run` override what's saved).
//
// Reads happen lazily on every access, so updating a value in the menu
// bar and saving takes effect on the very next transcription — no
// restart required.

let kKeychainService = "com.bezrabotnyi.voicepastefn"
let kKeychainAccountAPIKey = "openai_api_key"

let kDefaultsKeyBaseURL = "openai_base_url"
let kDefaultsKeyModel = "transcribe_model"
let kDefaultsKeyBaseURLSet = "openai_base_url_set"   // distinguishes "unset" from "= ''"

/// Default endpoint shown in the "Edit…" dialog the very first time.
let kDefaultBaseURL = "https://api.openai.com/v1"
let kDefaultModel = "whisper-1"

final class SettingsStore {
    static let shared = SettingsStore()
    private init() {}

    private let defaults = UserDefaults.standard

    // MARK: Base URL
    var baseURL: String {
        // Env override wins.
        if let env = ProcessInfo.processInfo.environment["OPENAI_BASE_URL"], !env.isEmpty {
            return env
        }
        if defaults.bool(forKey: kDefaultsKeyBaseURLSet),
           let saved = defaults.string(forKey: kDefaultsKeyBaseURL),
           !saved.isEmpty {
            return saved
        }
        return kDefaultBaseURL
    }

    func setBaseURL(_ value: String) throws {
        let trimmed = value.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !trimmed.isEmpty, URL(string: trimmed) != nil else {
            throw NSError(domain: "VoicePaste", code: 20,
                          userInfo: [NSLocalizedDescriptionKey: "Invalid URL: \(value)"])
        }
        defaults.set(trimmed, forKey: kDefaultsKeyBaseURL)
        defaults.set(true, forKey: kDefaultsKeyBaseURLSet)
    }

    // MARK: API key (Keychain)
    var apiKey: String {
        if let env = ProcessInfo.processInfo.environment["OPENAI_API_KEY"], !env.isEmpty {
            return env
        }
        return readKeychainAPIKey() ?? ""
    }

    func setAPIKey(_ value: String) throws {
        try writeKeychainAPIKey(value)
    }

    func clearAPIKey() {
        deleteKeychainAPIKey()
    }

    // MARK: Model (Whisper)
    var model: String {
        if let env = ProcessInfo.processInfo.environment["TRANSCRIBE_MODEL"], !env.isEmpty {
            return env
        }
        return defaults.string(forKey: kDefaultsKeyModel) ?? kDefaultModel
    }

    func setModel(_ value: String) {
        defaults.set(value, forKey: kDefaultsKeyModel)
    }

    // MARK: Display helpers
    var maskedBaseURL: String {
        // Show the host + first path segment so the user can tell which
        // endpoint they're talking to without exposing the full URL.
        let u = URL(string: baseURL)
        if let host = u?.host {
            return host
        }
        return baseURL
    }

    var maskedAPIKey: String {
        let k = apiKey
        guard !k.isEmpty else { return "(not set)" }
        if k.count <= 8 {
            return String(repeating: "•", count: k.count)
        }
        let prefix = k.prefix(3)
        let suffix = k.suffix(4)
        return "\(prefix)•••\(suffix)  (\(k.count) chars)"
    }

    var isConfigured: Bool {
        !baseURL.isEmpty && !apiKey.isEmpty
    }

    // MARK: - Keychain helpers (kSecClassGenericPassword)
    private func readKeychainAPIKey() -> String? {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    private func writeKeychainAPIKey(_ value: String) throws {
        // Delete any existing item first — SecItemUpdate can be finicky about
        // the data attribute on a fresh keychain.
        let baseQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
        ]
        SecItemDelete(baseQuery as CFDictionary)

        if value.isEmpty {
            return
        }
        var addQuery = baseQuery
        addQuery[kSecValueData as String] = value.data(using: .utf8) ?? Data()
        addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw NSError(
                domain: "VoicePaste", code: Int(status),
                userInfo: [NSLocalizedDescriptionKey:
                    "Keychain write failed (status \(status)). " +
                    "If the system keeps prompting for permission, allow VoicePasteFn " +
                    "in Keychain Access (System Settings → Privacy & Security)."]
            )
        }
    }

    private func deleteKeychainAPIKey() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
        ]
        SecItemDelete(query as CFDictionary)
    }
}
enum Language: String, CaseIterable {
    case ru
    case en
    case auto

    var title: String {
        switch self {
        case .ru: return "Russian / ru"
        case .en: return "English / en"
        case .auto: return "Auto"
        }
    }

    var apiValue: String? {
        switch self {
        case .ru: return "ru"
        case .en: return "en"
        case .auto: return nil
        }
    }
}

final class Settings {
    static let shared = Settings()

    private let defaults = UserDefaults.standard

    private enum Key {
        static let language = "language"
        static let realtimePreview = "realtimePreview"
        static let autostart = "autostart"
        static let selectedModel = "selectedModel"
        static let recordingDelay = "recordingDelay"
        static let hideDelay = "hideDelay"
        static let hotkey = "hotkey"
        static let activationMode = "activationMode"
        static let overlayCentered = "overlayCentered"
        static let wakeServerOnStart = "wakeServerOnStart"
        static let realtimeChunkInterval = "realtimeChunkInterval"
        static let appleFallback = "appleFallback"
    }

    private init() {
        if defaults.string(forKey: Key.language) == nil {
            defaults.set(Language.ru.rawValue, forKey: Key.language)
        }
    }

    var language: Language {
        get { Language(rawValue: defaults.string(forKey: Key.language) ?? "ru") ?? .ru }
        set { defaults.set(newValue.rawValue, forKey: Key.language) }
    }

    var realtimePreview: Bool {
        get { defaults.bool(forKey: Key.realtimePreview) }
        set { defaults.set(newValue, forKey: Key.realtimePreview) }
    }

    var autostart: Bool {
        get { defaults.bool(forKey: Key.autostart) }
        set { defaults.set(newValue, forKey: Key.autostart) }
    }

    var selectedModel: String {
        get { defaults.string(forKey: Key.selectedModel) ?? "auto" }
        set { defaults.set(newValue, forKey: Key.selectedModel) }
    }

    /// How long Fn must be held before recording actually starts (debounce).
    /// Range 0.10–2.00 seconds; default 0.20. Clamped on read so a corrupted
    /// or out-of-range value from disk never breaks the recorder.
    var recordingDelay: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.recordingDelay)
            // 0 means never set; treat as default.
            let raw = stored == 0 ? 0.20 : stored
            return min(2.0, max(0.1, raw))
        }
        set {
            let clamped = min(2.0, max(0.1, newValue))
            defaults.set(clamped, forKey: Key.recordingDelay)
        }
    }

    /// How long the overlay stays visible after the final transcript is pasted,
    /// so the user can read what got written to the clipboard.
    /// Range 0.0 – 5.0 seconds; default 0.8 (0 means "never hide automatically",
    /// user dismisses by holding Fn again or by clicking).
    var hideDelay: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.hideDelay)
            let raw = stored == 0 ? 0.8 : stored
            return min(5.0, max(0.0, raw))
        }
        set {
            let clamped = min(5.0, max(0.0, newValue))
            defaults.set(clamped, forKey: Key.hideDelay)
        }
    }

    /// Which hotkey triggers recording. Default: Fn.
    /// Stored as the rawValue string of HotkeyKind (see HotkeyKind enum).
    var hotkey: HotkeyKind {
        get {
            let raw = defaults.string(forKey: Key.hotkey) ?? HotkeyKind.fn.rawValue
            return HotkeyKind(rawValue: raw) ?? .fn
        }
        set { defaults.set(newValue.rawValue, forKey: Key.hotkey) }
    }

    /// How the hotkey triggers: hold (press to start, release to stop) or
    /// toggle (first press starts, second press stops). Default: hold.
    var activationMode: ActivationMode {
        get {
            let raw = defaults.string(forKey: Key.activationMode) ?? ActivationMode.hold.rawValue
            return ActivationMode(rawValue: raw) ?? .hold
        }
        set { defaults.set(newValue.rawValue, forKey: Key.activationMode) }
    }

    /// Whether to put the preview overlay at the cursor's position
    /// (default-ish, "follow mouse") or centred on screen.
    var overlayCentered: Bool {
        get { defaults.bool(forKey: Key.overlayCentered) }
        set { defaults.set(newValue, forKey: Key.overlayCentered) }
    }

    /// Ping the Whisper endpoint at dictation start so the server-side
    /// model isn't unloaded by idle-timeout between sessions. Default: true.
    var wakeServerOnStart: Bool {
        get {
            if defaults.object(forKey: Key.wakeServerOnStart) == nil { return true }
            return defaults.bool(forKey: Key.wakeServerOnStart)
        }
        set { defaults.set(newValue, forKey: Key.wakeServerOnStart) }
    }

    /// Realtime preview: how often (in seconds) to transcribe a chunk while
    /// the user is still holding the hotkey. Range 1.0 – 30.0 s; default 5.0.
    /// Smaller values feel more responsive but hit the API harder.
    var realtimeChunkInterval: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.realtimeChunkInterval)
            let raw = stored == 0 ? 5.0 : stored
            return min(30.0, max(1.0, raw))
        }
        set {
            let clamped = min(30.0, max(1.0, newValue))
            defaults.set(clamped, forKey: Key.realtimeChunkInterval)
        }
    }

    /// When enabled, if the Whisper server fails 3 times, fall back to
    /// Apple's on-device speech recognition for the same audio.
    var appleFallback: Bool {
        get { defaults.bool(forKey: Key.appleFallback) }
        set { defaults.set(newValue, forKey: Key.appleFallback) }
    }
}
