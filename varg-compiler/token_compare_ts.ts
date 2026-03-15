// Realistic scenario: API data processor with error handling (TypeScript)
class ApiProcessor {
    private baseUrl: string;
    private requestCount: number;

    constructor() {
        this.baseUrl = "https://api.example.com";
        this.requestCount = 0;
    }

    async fetchData(endpoint: string): Promise<string> {
        const url = `${this.baseUrl}/${endpoint}`;
        const response = await fetch(url);
        this.requestCount += 1;
        return await response.text();
    }

    processItems(jsonData: string): number {
        const items = JSON.parse(jsonData) as Record<string, unknown>;
        const keys = Object.keys(items);
        let total = 0;
        for (let idx = 0; idx < keys.length; idx++) {
            total += 1;
        }
        return total;
    }

    async run(): Promise<void> {
        console.log("Processing API data...");
        try {
            const data = await this.fetchData("items");
            const count = this.processItems(data);
            console.log(count);
        } catch (err) {
            console.log(err);
        }
    }
}

const processor = new ApiProcessor();
processor.run();
