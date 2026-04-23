mod _utils;

use indoc::indoc;

use macros::generate_tests;

generate_tests! {
    rule: collapsible_let_in,
    expressions: [
        indoc! {r"
            let
              a = 2;
              b = 3;
            in
              let
                c = 5;
                d = 6;
              in
              a + b + c + d
        "},

        indoc! {r"
            let
              x = f a;
            in
            let
              a = 1;
            in
            x
        "},

        indoc! {r"
            let
              xs = [a b c ];
            in
            let
              a = 1;
            in
            xs
        "},

        indoc! {r"
            let
              x = f y;
            in
            let
              z = 1;
            in
            x
        "},
        indoc! {r"
            {
              pkgs ? import <nixpkgs> { },
            }:
            let
              pkgs' = pkgs.extend (import ./overlay.nix);
            in
            let
              pkgs = pkgs';
            in
            pkgs
        "},
        indoc! {r"
            let
              a.b = 1;
            in
            let
              c = 2;
            in
            c
        "},
        indoc! {r"
            let
              inherit pkgs;
            in
            let
              pkgs = pkgs.extend (import ./overlay.nix);
            in
            pkgs
        "},
        indoc! {r"
            let
              inherit (foo) bar;
            in
            let
              bar = 1;
            in
            bar
        "},
        indoc! {r"
            let
              x = a.b;
            in
            let
              b = 1;
            in
            x
        "},
    ],
}
