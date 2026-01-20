"""
Aegis Database Python SDK Setup

@version 1.0.0
@author AutomataNexus Development Team
"""

from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as f:
    long_description = f.read() if f else ""

setup(
    name="aegis-db",
    version="1.0.0",
    author="AutomataNexus Development Team",
    author_email="dev@aegisdb.io",
    description="Official Python client for Aegis Database",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/aegisdb/aegis-db",
    project_urls={
        "Bug Tracker": "https://github.com/aegisdb/aegis-db/issues",
        "Documentation": "https://docs.aegisdb.io",
    },
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Database",
        "Topic :: Software Development :: Libraries :: Python Modules",
        "Typing :: Typed",
    ],
    packages=find_packages(),
    python_requires=">=3.9",
    install_requires=[
        "aiohttp>=3.8.0",
    ],
    extras_require={
        "pandas": ["pandas>=1.5.0"],
        "dev": [
            "pytest>=7.0.0",
            "pytest-asyncio>=0.21.0",
            "mypy>=1.0.0",
            "black>=23.0.0",
            "ruff>=0.0.270",
        ],
    },
    package_data={
        "aegis_db": ["py.typed"],
    },
)
