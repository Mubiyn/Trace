from fastapi import FastAPI

app = FastAPI()


@app.post("/api/calls")
def create_call():
    return {"status": "ok"}
