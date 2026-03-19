import networkx as nx
import numpy as np
from numpy.linalg import norm

def cosine_similarity(a, b):
    return np.dot(a, b) / (norm(a) * norm(b))

def embed(text, dim=384):
    np.random.seed(hash(text) % 2**32)
    vec = np.random.randn(dim)
    return vec / norm(vec)

graph = nx.DiGraph()
graph.add_node("rust", type="language", paradigm="systems")
graph.add_node("varg", type="language", paradigm="agent")
graph.add_edge("varg", "rust", relation="transpiles_to")

vector_store = {}
embedding = embed("Varg compiles to native code via Rust")
vector_store["doc1"] = {"vector": embedding, "meta": {"source": "readme"}}

query_vec = embed("How does Varg compile?")
results = sorted(
    vector_store.items(),
    key=lambda item: cosine_similarity(query_vec, item[1]["vector"]),
    reverse=True,
)[:3]

neighbors = list(graph.neighbors("varg"))
traversal = list(nx.bfs_tree(graph, "varg", depth_limit=2))

print(f"Found {len(results)} similar docs")
print(f"Graph neighbors: {len(neighbors)}")
