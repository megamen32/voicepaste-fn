import Foundation

public enum ModelListClientError: Error, LocalizedError {
    case invalidBaseURL
    case noHTTPResponse
    case httpStatus(Int)
    case invalidResponse

    public var errorDescription: String? {
        switch self {
        case .invalidBaseURL:
            return "Invalid model endpoint URL"
        case .noHTTPResponse:
            return "No HTTP response from model endpoint"
        case .httpStatus(let status):
            return "Model endpoint returned HTTP \(status)"
        case .invalidResponse:
            return "Model endpoint returned invalid JSON"
        }
    }
}

/// HTTP client for the OpenAI-compatible GET /models contract.
public struct ModelListClient {
    private let baseURL: URL
    private let apiKey: String

    public init(baseURL: String, apiKey: String = "") throws {
        let trimmed = baseURL.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !trimmed.isEmpty, let url = URL(string: trimmed) else {
            throw ModelListClientError.invalidBaseURL
        }
        self.baseURL = url
        self.apiKey = apiKey
    }

    public func fetchModels() throws -> [String] {
        var request = URLRequest(url: baseURL.appendingPathComponent("models"))
        request.httpMethod = "GET"
        request.timeoutInterval = 10
        if !apiKey.isEmpty {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        let semaphore = DispatchSemaphore(value: 0)
        var resultData: Data?
        var resultResponse: URLResponse?
        var resultError: Error?

        URLSession.shared.dataTask(with: request) { data, response, error in
            resultData = data
            resultResponse = response
            resultError = error
            semaphore.signal()
        }.resume()
        semaphore.wait()

        if let error = resultError {
            throw error
        }
        guard let http = resultResponse as? HTTPURLResponse else {
            throw ModelListClientError.noHTTPResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            throw ModelListClientError.httpStatus(http.statusCode)
        }

        guard let data = resultData,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let models = json["data"] as? [[String: Any]] else {
            throw ModelListClientError.invalidResponse
        }

        return models.compactMap { $0["id"] as? String }.sorted()
    }
}
