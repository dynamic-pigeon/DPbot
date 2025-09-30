import sys
from typing import List, Tuple
import requests
from bs4 import BeautifulSoup
from datetime import datetime
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
from io import BytesIO


RATING_COLORS = [
    (400, "#808080", "灰色"),
    (800, "#A5612A", "棕色"),
    (1200, "#008000", "绿色"),
    (1600, "#00FFFF", "青色"),
    (2000, "#0000FF", "蓝色"),
    (2400, "#FFFF00", "黄色"),
    (2800, "#FFA500", "橙色"),
    (float(6000), "#FF0000", "红色"),
]


def fetch_history(ac_id: str) -> str:
    """Fetches the rating history page from AtCoder."""
    url = f"https://atcoder.jp/users/{ac_id}/history"
    try:
        response = requests.get(url)
        response.raise_for_status()
        return response.text
    except requests.exceptions.RequestException as e:
        print(f"无法连接到 AtCoder 服务器: {e}", file=sys.stderr)
        sys.exit(1)


def parse_history(html_content: str) -> Tuple[List[datetime], List[int]]:
    """Parses the HTML to extract dates and ratings."""
    soup = BeautifulSoup(html_content, "html.parser")
    table = soup.find(
        "table",
        class_="table table-default table-striped table-hover table-condensed table-bordered",
    )
    if not table:
        print("无法找到历史记录表格", file=sys.stderr)
        sys.exit(1)

    lst = table.find("tbody").find_all("tr")
    if not lst:
        print("没有参赛记录", file=sys.stderr)
        sys.exit(1)

    dates: List[datetime] = []
    ratings: List[int] = []
    for tr in lst:
        tds = tr.find_all("td")
        if tds[4].text == "-":
            continue

        ratings.append(int(tds[4].text))
        dates.append(datetime.strptime(tds[0].text.strip(), "%Y-%m-%d %H:%M:%S%z"))

    return dates, ratings


def plot_history(ac_id: str, dates: List[datetime], ratings: List[int]) -> bytes:
    """Generates a rating history plot and returns it as bytes."""
    fig, ax = plt.subplots(dpi=300, figsize=(10, 5))

    # Plot data
    ax.plot(
        dates,
        ratings,
        "o-",
        color="#4169E1",
        alpha=0.8,
        linewidth=1,
        label="rating",
        markersize=2,
    )

    # Set titles and labels
    ax.set_title(f"{ac_id}'s rating change")
    ax.set_xlabel("Time")
    ax.set_ylabel("Rating")

    # Set y-axis limits
    min_rating, max_rating = min(ratings), max(ratings)
    gap = (max_rating - min_rating) * 0.1
    y_min = min_rating - gap
    y_max = max_rating + gap
    ax.set_ylim(y_min, y_max)

    # Color background by rating
    lower_bound = y_min
    for upper_bound, color, _ in RATING_COLORS:
        ax.axhspan(lower_bound, upper_bound, facecolor=color, alpha=0.5)
        lower_bound = upper_bound
        if lower_bound > y_max:
            break

    # Format x-axis
    ax.xaxis.set_major_formatter(mdates.DateFormatter("%y-%m-%d"))
    plt.setp(ax.get_xticklabels(), rotation=20, ha="right")

    ax.legend()
    fig.tight_layout()

    # Save to buffer
    with BytesIO() as buffer:
        fig.savefig(buffer, format="png")
        buffer.seek(0)
        return buffer.read()


def main():
    """Main function to run the script."""
    if len(sys.argv) < 2:
        print("Usage: python at.py <AtCoder_ID>", file=sys.stderr)
        sys.exit(1)

    ac_id = sys.argv[1]
    html_content = fetch_history(ac_id)
    dates, ratings = parse_history(html_content)
    image_bytes = plot_history(ac_id, dates, ratings)
    sys.stdout.buffer.write(image_bytes)


if __name__ == "__main__":
    main()
