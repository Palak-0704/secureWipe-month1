import os
import requests

def ask_groq(prompt, api_key, model="llama2-70b-4096"):  # You can use other models like mixtral-8x7b-32768
    url = "https://api.groq.com/openai/v1/chat/completions"
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }
    data = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a helpful assistant for secure data wiping."},
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 256,
        "temperature": 0.7
    }
    response = requests.post(url, headers=headers, json=data)
    response.raise_for_status()
    return response.json()["choices"][0]["message"]["content"]

if __name__ == "__main__":
    api_key = os.getenv("GROQ_API_KEY")
    if not api_key:
        api_key = input("Enter your Groq API key: ")
    print("Secure Wipe AI Chatbot (Groq, Llama-2)")
    print("Type your question and press Enter. Type 'exit' to quit.\n")
    while True:
        prompt = input("You: ").strip()
        if prompt.lower() == "exit":
            print("Goodbye!")
            break
        try:
            answer = ask_groq(prompt, api_key)
            print(f"Bot: {answer}\n")
        except Exception as e:
            print(f"Error: {e}\n")
