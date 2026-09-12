package com.example;

class Point {
    int x, y;
    int dist() { return x + y; }
}

public class Main {
    public static void main(String[] args) {
        Point p = new Point();
        p.dist();
    }
}
