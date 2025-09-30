import sys
from typing import List, Dict, Any
import requests
from io import BytesIO
import matplotlib.pyplot as plt

# Configuration for rating ranges and colors
# Each tuple is (start_rating, number_of_100_point_bins, color)
RATING_CONFIG = [
    (800, 4, "gray"),
    (1200, 2, "g"),
    (1400, 2, "c"),
    (1600, 3, "b"),
    (1900, 2, "purple"),
    (2100, 3, "orange"),
    (2400, 12, "red"),
]
MIN_RATING = 800
MAX_RATING = 3500
NUM_BINS = (MAX_RATING - MIN_RATING) // 100 + 1


def fetch_user_status(cf_id: str) -> List[Dict[str, Any]]:
    """Fetches user submission status from the Codeforces API."""
    url = f"https://codeforces.com/api/user.status?handle={cf_id}"
    try:
        response = requests.get(url)
        response.raise_for_status()  # Raise an exception for bad status codes
        data = response.json()
        if data.get("status") != "OK":
            print(f"API Error: {data.get('comment', 'Unknown error')}", file=sys.stderr)
            sys.exit(1)
        return data.get("result", [])
    except requests.exceptions.RequestException as e:
        print(f"无法连接到 Codeforces 服务器: {e}", file=sys.stderr)
        sys.exit(1)


def process_submissions(
    submissions: List[Dict[str, Any]],
) -> tuple[Dict[int, int], int]:
    """Processes submissions to count unique AC problems per rating."""
    if not submissions:
        print("没有提交记录", file=sys.stderr)
        sys.exit(1)

    ac_problems = {}
    frequencies = {rating: 0 for rating in range(MIN_RATING, MAX_RATING + 100, 100)}

    for sub in submissions:
        problem = sub.get("problem", {})
        problem_id = f"{problem.get('contestId')}{problem.get('index')}"

        if (
            sub.get("verdict") == "OK"
            and "rating" in problem
            and problem_id not in ac_problems
        ):
            rating = problem["rating"]
            if MIN_RATING <= rating <= MAX_RATING:
                # Group ratings into 100-point bins
                bin_rating = (rating // 100) * 100
                frequencies[bin_rating] += 1
                ac_problems[problem_id] = True

    return frequencies, len(ac_problems)


def plot_analysis(cf_id: str, frequencies: Dict[int, int], total_ac: int) -> bytes:
    """Generates a bar chart of solved problems by rating."""
    fig, ax = plt.subplots(dpi=300, figsize=(10, 5))

    ratings = sorted(frequencies.keys())
    counts = [frequencies[r] for r in ratings]

    colors = []
    for _, count, color in RATING_CONFIG:
        colors.extend([color] * count)

    bar_width = 96
    ax.bar(
        ratings,
        counts,
        width=bar_width,
        color=colors,
        edgecolor=colors,
        antialiased=True,
    )

    ax.set_title(f"{cf_id} solved {total_ac} problems in total")
    ax.set_xlabel("Rating")
    ax.set_ylabel("Frequency")
    ax.set_xlim(MIN_RATING - 50, MAX_RATING + 50)

    fig.tight_layout()

    with BytesIO() as buffer:
        fig.savefig(buffer, format="png")
        return buffer.getvalue()


def main():
    """Main function to run the analysis."""
    if len(sys.argv) < 2:
        print("Usage: python analyze.py <Codeforces_ID>", file=sys.stderr)
        sys.exit(1)

    cf_id = sys.argv[1]
    submissions = fetch_user_status(cf_id)
    frequencies, total_ac = process_submissions(submissions)
    image_bytes = plot_analysis(cf_id, frequencies, total_ac)
    sys.stdout.buffer.write(image_bytes)


if __name__ == "__main__":
    main()
