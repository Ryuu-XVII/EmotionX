import sys
import json
from pathlib import Path
from graphify.extract import collect_files, extract
from graphify.build import build_from_json
from graphify.cluster import cluster, score_all
from graphify.analyze import god_nodes, surprising_connections, suggest_questions
from graphify.report import generate
from graphify.export import to_json, to_html
from graphify.diagnostics import diagnose_extraction, format_diagnostic_report
from graphify.detect import save_manifest, detect
from datetime import datetime, timezone

def main():
    detect_result = detect(Path('.'))
    Path('graphify-out/.graphify_detect.json').write_text(json.dumps(detect_result, ensure_ascii=False), encoding='utf-8')

    code_files = []
    for f in detect_result.get('files', {}).get('code', []):
        p = Path(f)
        code_files.extend(collect_files(p) if p.is_dir() else [p])

    print(f"Collecting {len(code_files)} code files for extraction...")
    if code_files:
        ast_result = extract(code_files, cache_root=Path('.'))
        print(f"AST: {len(ast_result['nodes'])} nodes, {len(ast_result['edges'])} edges")
    else:
        ast_result = {'nodes': [], 'edges': [], 'input_tokens': 0, 'output_tokens': 0}

    Path('graphify-out/.graphify_ast.json').write_text(json.dumps(ast_result, indent=2, ensure_ascii=False), encoding='utf-8')

    sem_result = {'nodes': [], 'edges': [], 'hyperedges': [], 'input_tokens': 0, 'output_tokens': 0}
    Path('graphify-out/.graphify_semantic.json').write_text(json.dumps(sem_result, indent=2, ensure_ascii=False), encoding='utf-8')

    merged_nodes = list(ast_result['nodes'])
    merged_edges = list(ast_result['edges'])
    merged = {
        'nodes': merged_nodes,
        'edges': merged_edges,
        'hyperedges': [],
        'input_tokens': 0,
        'output_tokens': 0,
    }
    Path('graphify-out/.graphify_extract.json').write_text(json.dumps(merged, indent=2, ensure_ascii=False), encoding='utf-8')

    G = build_from_json(merged, root='.', directed=False)
    print(f"Graph built: {G.number_of_nodes()} nodes, {G.number_of_edges()} edges")

    communities = cluster(G)
    cohesion = score_all(G, communities)
    tokens = {'input': 0, 'output': 0}
    gods = god_nodes(G)
    surprises = surprising_connections(G, communities)
    labels = {cid: f"Community {cid}" for cid in communities}
    questions = suggest_questions(G, communities, labels)

    to_json(G, communities, 'graphify-out/graph.json', force=True)
    to_html(G, communities, 'graphify-out/graph.html', community_labels=labels)

    report = generate(G, communities, cohesion, labels, gods, surprises, detect_result, tokens, '.', suggested_questions=questions)
    Path('graphify-out/GRAPH_REPORT.md').write_text(report, encoding='utf-8')

    analysis = {
        'communities': {str(k): v for k, v in communities.items()},
        'cohesion': {str(k): v for k, v in cohesion.items()},
        'gods': gods,
        'surprises': surprises,
        'questions': questions,
    }
    Path('graphify-out/.graphify_analysis.json').write_text(json.dumps(analysis, indent=2, ensure_ascii=False), encoding='utf-8')
    save_manifest(detect_result.get('all_files') or detect_result['files'], root='.')

    print("Graphify run complete! Generated graph.json, graph.html, and GRAPH_REPORT.md.")

if __name__ == '__main__':
    main()
